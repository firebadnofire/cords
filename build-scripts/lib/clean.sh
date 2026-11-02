#!/usr/bin/env bash
# Shared repository-only cleanup for Linux and macOS entry points.

clean_repo_outputs() {
  local root="$1" dry_run="$2"
  local relative target component prefix tracked
  local paths=(
    target
    dist
    .cords-build-cache
    .qt-sdk
    .qt-tools
    bins/cords-client/ui/node_modules
    bins/cords-client/ui/dist
    bins/cords-client/ui/.svelte-kit
    bins/cords-client/ui/public/avatar-codec
    bins/cords-client/src-tauri/gen
    bins/cords-client/src-tauri/target
    scripts/__pycache__
  )

  command -v git >/dev/null 2>&1 || { printf 'error: git is required for safe cleanup\n' >&2; return 1; }
  git -C "$root" rev-parse --is-inside-work-tree >/dev/null 2>&1 || {
    printf 'error: cleanup requires a Git checkout: %s\n' "$root" >&2
    return 1
  }

  # Preflight every path before deleting any output. Refuse tracked content and
  # symlinks at any level; never infer deletable paths from git clean -X.
  for relative in "${paths[@]}"; do
    target="$root/$relative"
    prefix="$root"
    IFS=/ read -r -a components <<< "$relative"
    for component in "${components[@]}"; do
      prefix="$prefix/$component"
      if [[ -L "$prefix" ]]; then
        printf 'error: cleanup path contains a symlink: %s\n' "$prefix" >&2
        return 1
      fi
    done
    if [[ -e "$target" && ! -d "$target" ]]; then
      printf 'error: cleanup path is not a directory: %s\n' "$target" >&2
      return 1
    fi
    tracked="$(git -C "$root" ls-files -- "$relative")"
    if [[ -n "$tracked" ]]; then
      printf 'error: cleanup path contains tracked files: %s\n' "$relative" >&2
      return 1
    fi
  done

  for relative in "${paths[@]}"; do
    target="$root/$relative"
    if [[ ! -d "$target" ]]; then
      printf 'Already clean: %s\n' "$relative"
    elif [[ "$dry_run" == true ]]; then
      printf 'Would remove: %s\n' "$relative"
    else
      printf 'Cleaning: %s\n' "$relative"
      rm -rf -- "$target"
    fi
  done
}
