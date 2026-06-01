#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

VERIFY_REPO="$(mktemp -d "${TMPDIR:-/tmp}/rickydata-git-verify.XXXXXX")"
cleanup() {
  rm -rf "$VERIFY_REPO"
}
trap cleanup EXIT

cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/verify-deploy-config.sh

cargo run -p rickydata-git-cli -- doctor --json
cargo run -p rickydata-git-cli -- init --repo "$REPO_ROOT" --json
cargo run -p rickydata-git-cli -- inspect --repo "$REPO_ROOT" --json
cargo run -p rickydata-git-cli -- status --repo "$REPO_ROOT" --json
OBJECT_ID="$(
  cargo run -p rickydata-git-cli -- object write \
    --repo "$REPO_ROOT" \
    --kind agent.intent \
    --body-file "$REPO_ROOT/fixtures/work-intent.valid.json" \
    --json |
    jq -r '.object_id'
)"
cargo run -p rickydata-git-cli -- object verify \
  --repo "$REPO_ROOT" \
  --object-id "$OBJECT_ID" \
  --json >/tmp/rickydata-git-object-verify.json
cargo run -p rickydata-git-cli -- discovery --repo "$REPO_ROOT" --json >/tmp/rickydata-git-discovery.json
cargo run -p rickydata-git-cli -- manifest --json >/tmp/rickydata-git-manifest.json
cargo run -p rickydata-git-cli -- schema --json >/tmp/rickydata-git-schema.json

git -C "$VERIFY_REPO" init -b main
git -C "$VERIFY_REPO" config user.email agent@example.com
git -C "$VERIFY_REPO" config user.name Agent
printf "# verify\n" >"$VERIFY_REPO/README.md"
git -C "$VERIFY_REPO" add README.md
git -C "$VERIFY_REPO" commit -m initial

cargo run -p rickydata-git-cli -- init --repo "$VERIFY_REPO" --json |
  jq -e '.status == "created" or .status == "already_initialized"' >/dev/null
INTENT_ID="$(
  cargo run -p rickydata-git-cli -- intent write \
    --repo "$VERIFY_REPO" \
    "$REPO_ROOT/fixtures/work-intent.valid.json" \
    --json |
    jq -r -e '.object.object_id'
)"
ATTEMPT_ID="$(
  cargo run -p rickydata-git-cli -- attempt start \
    --repo "$VERIFY_REPO" \
    --intent-id "$INTENT_ID" \
    --agent-id agent:verify \
    --idempotency-key verify \
    --json |
    jq -r -e '.attempt.attempt_id'
)"
RUN_ID="$(
  cargo run -p rickydata-git-cli -- run exec \
    --repo "$VERIFY_REPO" \
    --attempt-id "$ATTEMPT_ID" \
    --json \
    -- sh -c "printf 'verified\n' > generated.txt" |
    jq -r -e '.run.run_id'
)"
CHANGE_ID="$(
  cargo run -p rickydata-git-cli -- change detect \
    --repo "$VERIFY_REPO" \
    --attempt-id "$ATTEMPT_ID" \
    --json |
    jq -r -e '.change.change_id'
)"
cargo run -p rickydata-git-cli -- change list --repo "$VERIFY_REPO" --json |
  jq -e --arg change_id "$CHANGE_ID" '.changes | map(.change.change_id) | index($change_id)' >/dev/null
cargo run -p rickydata-git-cli -- change show \
  --repo "$VERIFY_REPO" \
  --change-id "$CHANGE_ID" \
  --json |
  jq -e --arg run_id "$RUN_ID" '.change.run_ids | index($run_id)' >/dev/null
PATCH_OBJECT_ID="$(
  cargo run -p rickydata-git-cli -- patch prepare \
    --repo "$VERIFY_REPO" \
    --attempt-id "$ATTEMPT_ID" \
    --json |
    jq -r -e '.object.object_id'
)"
PATCH_ID="$(
  cargo run -p rickydata-git-cli -- object read \
    --repo "$VERIFY_REPO" \
    --object-id "$PATCH_OBJECT_ID" \
    --json |
    jq -r -e 'select(.object.kind == "agent.patch") | .object.body.patch_id'
)"
cargo run -p rickydata-git-cli -- patch list --repo "$VERIFY_REPO" --json |
  jq -e --arg patch_id "$PATCH_ID" '.patches | map(.patch.patch_id) | index($patch_id)' >/dev/null
cargo run -p rickydata-git-cli -- patch show \
  --repo "$VERIFY_REPO" \
  --patch-id "$PATCH_ID" \
  --json |
  jq -e --arg change_id "$CHANGE_ID" '.patch.change_ids | index($change_id)' >/dev/null
PATCH_FILE="$VERIFY_REPO/.git/rickydata/tmp/verify.patch"
cargo run -p rickydata-git-cli -- patch export \
  --repo "$VERIFY_REPO" \
  --patch-id "$PATCH_ID" \
  --output "$PATCH_FILE" \
  --json |
  jq -e --arg patch_id "$PATCH_ID" '.patch_id == $patch_id and .diff_bytes > 0' >/dev/null
git -C "$VERIFY_REPO" apply --check "$PATCH_FILE"
rm -rf "$VERIFY_REPO/.git/rickydata/cache/objects"
RECOVERED_PATCH_FILE="$VERIFY_REPO/.git/rickydata/tmp/recovered.patch"
cargo run -p rickydata-git-cli -- patch export \
  --repo "$VERIFY_REPO" \
  --patch-id "$PATCH_ID" \
  --output "$RECOVERED_PATCH_FILE" \
  --json |
  jq -e --arg patch_id "$PATCH_ID" '.patch_id == $patch_id and .diff_bytes > 0' >/dev/null
cmp "$PATCH_FILE" "$RECOVERED_PATCH_FILE"
CHECKOUT_PATH="$(
  cargo run -p rickydata-git-cli -- patch checkout \
    --repo "$VERIFY_REPO" \
    --patch-id "$PATCH_ID" \
    --json |
    jq -r -e 'select(.applied == true and .diff_bytes > 0) | .checkout_path'
)"
test "$(cat "$CHECKOUT_PATH/generated.txt")" = "verified"
test -z "$(git -C "$VERIFY_REPO" status --short)"
cargo run -p rickydata-git-cli -- sync verify --repo "$VERIFY_REPO" --json |
  jq -e '.status == "ok" and .object_count == .valid_object_count and .patch_count == .valid_patch_count' >/dev/null
cargo run -p rickydata-git-cli -- object read \
  --repo "$VERIFY_REPO" \
  --object-id "$PATCH_OBJECT_ID" \
  --json |
  jq -e '.object.kind == "agent.patch"' >/dev/null
test -z "$(git -C "$VERIFY_REPO" status --short)"
git -C "$VERIFY_REPO" fsck --no-dangling

git fsck --no-dangling
