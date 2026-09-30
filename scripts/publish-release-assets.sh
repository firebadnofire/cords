#!/usr/bin/env bash
set -euo pipefail

[[ $# -eq 1 ]] || {
  echo "usage: $0 <release-directory>" >&2
  exit 2
}

release_directory="$1"
[[ -d "${release_directory}" ]] || {
  echo "error: release directory does not exist: ${release_directory}" >&2
  exit 1
}
release_directory="$(cd "${release_directory}" && pwd)"

: "${RELEASE_TAG:?RELEASE_TAG is required}"
: "${RELEASE_NAME:?RELEASE_NAME is required}"
: "${RELEASE_BODY:?RELEASE_BODY is required}"
: "${SOURCE_REVISION:?SOURCE_REVISION is required}"
: "${FORGEJO_API_URL:?FORGEJO_API_URL is required}"
: "${FORGEJO_REPOSITORY:?FORGEJO_REPOSITORY is required}"
: "${FORGEJO_TOKEN:?FORGEJO_TOKEN is required}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
: "${GH_KEY:?GH_KEY is required}"

mapfile -d '' release_assets < <(
  find "${release_directory}" -maxdepth 1 -type f -print0 | sort -z
)
(( ${#release_assets[@]} > 0 )) || {
  echo "error: no release assets were found" >&2
  exit 1
}

request_json() {
  local authorization="$1"
  local method="$2"
  local url="$3"
  local body="${4:-}"
  local output="$5"
  local -a arguments=(
    --proto '=https' --tlsv1.2 --silent --show-error
    --request "${method}"
    --header "Authorization: ${authorization}"
    --header 'Accept: application/json'
    --header 'Content-Type: application/json'
    --output "${output}"
    --write-out '%{http_code}'
  )
  if [[ -n "${body}" ]]; then
    arguments+=(--data "${body}")
  fi
  curl "${arguments[@]}" "${url}"
}

ensure_release() {
  local provider="$1"
  local api_base="$2"
  local repository="$3"
  local authorization="$4"
  local response_file status payload release_id

  response_file="$(mktemp)"
  status="$(request_json "${authorization}" GET \
    "${api_base}/repos/${repository}/releases/tags/${RELEASE_TAG}" '' "${response_file}")"
  case "${status}" in
    200) ;;
    404)
      payload="$(jq -n \
        --arg tag "${RELEASE_TAG}" \
        --arg name "${RELEASE_NAME}" \
        --arg body "${RELEASE_BODY}" \
        --arg revision "${SOURCE_REVISION}" \
        '{tag_name:$tag,target_commitish:$revision,name:$name,body:$body,draft:false,prerelease:false}')"
      status="$(request_json "${authorization}" POST \
        "${api_base}/repos/${repository}/releases" "${payload}" "${response_file}")"
      [[ "${status}" == 201 ]] || {
        echo "error: ${provider} release creation returned HTTP ${status}" >&2
        rm -f -- "${response_file}"
        return 1
      }
      ;;
    *)
      echo "error: ${provider} release lookup returned HTTP ${status}" >&2
      rm -f -- "${response_file}"
      return 1
      ;;
  esac

  release_id="$(jq -er '.id' "${response_file}")"
  printf '%s\t%s\n' "${release_id}" "${response_file}"
}

delete_asset() {
  local provider="$1"
  local api_base="$2"
  local repository="$3"
  local authorization="$4"
  local asset_id="$5"
  local output status
  output="$(mktemp)"
  status="$(request_json "${authorization}" DELETE \
    "${api_base}/repos/${repository}/releases/assets/${asset_id}" '' "${output}")"
  rm -f -- "${output}"
  [[ "${status}" == 204 ]] || {
    echo "error: ${provider} asset deletion returned HTTP ${status}" >&2
    return 1
  }
}

upload_assets() {
  local provider="$1"
  local api_base="$2"
  local repository="$3"
  local authorization="$4"
  local release_id="$5"
  local release_json="$6"
  local upload_base="$7"
  local artifact name encoded_name existing_id

  for artifact in "${release_assets[@]}"; do
    name="$(basename "${artifact}")"
    existing_id="$(jq -r --arg name "${name}" '.assets[]? | select(.name == $name) | .id' "${release_json}")"
    if [[ -n "${existing_id}" ]]; then
      delete_asset "${provider}" "${api_base}" "${repository}" "${authorization}" "${existing_id}"
    fi
    encoded_name="$(jq -nr --arg value "${name}" '$value | @uri')"
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
      --request POST \
      --header "Authorization: ${authorization}" \
      --header 'Content-Type: application/octet-stream' \
      --data-binary "@${artifact}" \
      "${upload_base}${encoded_name}" >/dev/null
    echo "Published ${provider} release asset: ${name}"
  done
}

forgejo_result="$(ensure_release forgejo "${FORGEJO_API_URL%/}" \
  "${FORGEJO_REPOSITORY}" "token ${FORGEJO_TOKEN}")"
forgejo_release_id="${forgejo_result%%$'\t'*}"
forgejo_json="${forgejo_result#*$'\t'}"
upload_assets forgejo "${FORGEJO_API_URL%/}" "${FORGEJO_REPOSITORY}" \
  "token ${FORGEJO_TOKEN}" "${forgejo_release_id}" "${forgejo_json}" \
  "${FORGEJO_API_URL%/}/repos/${FORGEJO_REPOSITORY}/releases/${forgejo_release_id}/assets?name="
rm -f -- "${forgejo_json}"

github_api='https://api.github.com'
github_result="$(ensure_release github "${github_api}" \
  "${GITHUB_REPOSITORY}" "Bearer ${GH_KEY}")"
github_release_id="${github_result%%$'\t'*}"
github_json="${github_result#*$'\t'}"
github_upload_url="$(jq -er '.upload_url | sub("\\{.*$"; "")' "${github_json}")"
upload_assets github "${github_api}" "${GITHUB_REPOSITORY}" \
  "Bearer ${GH_KEY}" "${github_release_id}" "${github_json}" \
  "${github_upload_url}?name="
rm -f -- "${github_json}"
