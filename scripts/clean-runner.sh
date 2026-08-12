#!/usr/bin/env bash
# clean-runner.sh — 清理 MCPOrb Runner 环境，为 TestFlight 干净安装测试准备
#
# 只清理对「干净安装」必要的部分（App 卸载 + 用户数据 + 钥匙串）：
#   1. 卸载 /Applications/MCPOrb Runner.app（root 属主时请求管理员权限）
#   2. 删除沙盒容器 ~/Library/Containers/com.mcporb.runner（数据部分；TCC 保护
#      的元数据标记由 containermanagerd 在登出/重启后自动回收，不影响测试）
#   3. 删除 Orb Registry ~/Library/Application Support/MCPOrb
#   4. 删除 4 组 Runner 缓存 + 偏好 plist
#   5. 删除钥匙串 com.mcporb.orb-unlock 条目（卸载 App 后仍残留的隐形状态）
#
# 不包含（非必要）：target/ 构建缓存。如需一并清理构建产物，加 --build-cache。
#   注意：--build-cache 与 TestFlight 分发无关，仅释放磁盘并强制全量重编。
#
# 用法：
#   bash clean-runner.sh           # 执行必要清理
#   bash clean-runner.sh --check   # 只读预览，不删除任何东西
#   bash clean-runner.sh --build-cache   # 必要清理 + cargo clean 三个 workspace
set -uo pipefail

APP="/Applications/MCPOrb Runner.app"
CONTAINER="$HOME/Library/Containers/com.mcporb.runner"
APP_SUPPORT="$HOME/Library/Application Support/MCPOrb"
PREFS="$HOME/Library/Preferences/com.mcporb.runner.plist"
KEYCHAIN_SVCE="com.mcporb.orb-unlock"

CACHES=(
  "$HOME/Library/Caches/com.mcporb.runner"
  "$HOME/Library/Caches/mcporb-runner"
  "$HOME/Library/Caches/com.mcporb.runtime"
  "$HOME/Library/Caches/mcporb-runtime-app"
)

log()  { printf '%s\n' "$*"; }
warn() { printf '  ⚠ %s\n' "$*" >&2; }

# dump 按 "keychain:" 行分隔每个 item 块；acct 与 svce 同块解析，避免错配
keychain_accounts() {
  python3 - "$KEYCHAIN_SVCE" <<'PYEOF'
import re, subprocess, sys
svce = sys.argv[1]
out = subprocess.run(['security', 'dump-keychain'], capture_output=True, text=True).stdout
for item in re.split(r'(?m)^keychain: ', out):
    s = re.search(r'"svce"<blob>="([^"]*)"', item)
    if s and s.group(1) == svce:
        a = re.search(r'"acct"<blob>="([^"]*)"', item)
        if a:
            print(a.group(1))
PYEOF
}

uninstall_app() {
  if [[ ! -e "$APP" ]]; then
    log "  · App 未安装，跳过"
    return
  fi
  if (( CHECK_MODE )); then
    log "  [预览] 卸载 $APP"
    return
  fi
  # 先以当前用户尝试；MAS/TestFlight 安装为 root 属主，失败则请求管理员
  if rm -rf -- "$APP" 2>/dev/null; then
    log "  ✓ 已卸载 $APP"
  else
    log "  → 需要管理员权限卸载 ${APP}（macOS 将弹出密码框）"
    if osascript -e 'do shell script "rm -rf \"/Applications/MCPOrb Runner.app\"" with administrator privileges' >/dev/null 2>&1; then
      log "  ✓ 已卸载 $APP"
    else
      warn "App 卸载失败（授权被取消或路径被占用）"
    fi
  fi
}

# 容器根含 TCC 保护元数据（rm 整体会失败）；能删的删掉，外壳留给 containermanagerd
clean_container() {
  if [[ ! -e "$CONTAINER" ]]; then
    log "  · 容器不存在，跳过"
    return
  fi
  if (( CHECK_MODE )); then
    log "  [预览] 删除容器数据 $CONTAINER"
    return
  fi
  local remaining
  if rm -rf -- "$CONTAINER" 2>/dev/null; then
    log "  ✓ 容器已删除"
    return
  fi
  rm -rf -- "$CONTAINER"/Data "$CONTAINER"/tmp "$CONTAINER"/SystemData 2>/dev/null || true
  remaining="$(ls -A "$CONTAINER" 2>/dev/null | tr '\n' ' ')"
  if [[ -z "$remaining" ]]; then
    rmdir "$CONTAINER" 2>/dev/null && log "  ✓ 容器已删除" || warn "容器元数据标记受保护，登出/重启后由 containermanagerd 回收（不影响干净安装）"
  else
    warn "容器内仍有内容: $remaining"
  fi
}

clean_keychain() {
  local accts count a
  accts="$(keychain_accounts)"
  if [[ -z "$accts" ]]; then
    log "  · 钥匙串无 $KEYCHAIN_SVCE 条目，跳过"
    return
  fi
  if (( CHECK_MODE )); then
    log "  [预览] 删除钥匙串条目:"
    while IFS= read -r a; do [[ -n "$a" ]] && log "    - $KEYCHAIN_SVCE (acct: $a)"; done <<< "$accts"
    return
  fi
  count=0
  while IFS= read -r a; do
    [[ -z "$a" ]] && continue
    if security delete-generic-password -s "$KEYCHAIN_SVCE" -a "$a" >/dev/null 2>&1; then
      count=$((count + 1))
    else
      warn "删除失败: $KEYCHAIN_SVCE acct=$a"
    fi
  done <<< "$accts"
  log "  ✓ 已删除 $count 条钥匙串条目"
}

clean_build_cache() {
  (( DO_BUILD_CACHE )) || return 0
  local root ws
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  for ws in "$root" "$root/../MCPOrbBuilder" "$root/../MCPOrb_HEAD_release_verify"; do
    if [[ -d "$ws/target" ]]; then
      if (( CHECK_MODE )); then
        log "  [预览] cargo clean $ws"
      else
        log "  → cargo clean $ws"
        ( cd "$ws" && cargo clean ) >/dev/null 2>&1 || warn "cargo clean 失败: $ws"
      fi
    fi
  done
}

main() {
  local c leftovers=()
  CHECK_MODE=0
  DO_BUILD_CACHE=0
  for arg in "$@"; do
    case "$arg" in
      --check) CHECK_MODE=1 ;;
      --build-cache) DO_BUILD_CACHE=1 ;;
      -h|--help) sed -n '1,30p' "$0" | grep '^#'; exit 0 ;;
      *) echo "未知参数: ${arg}（支持: --check / --build-cache / --help）" >&2; exit 2 ;;
    esac
  done

  log "== MCPOrb Runner 环境清理 =="
  (( CHECK_MODE )) && log "模式: 只读预览（不会删除任何内容）"

  log "[1/4] 卸载 App"
  uninstall_app
  log "[2/4] 删除用户数据"
  clean_container
  if [[ -e "$APP_SUPPORT" ]]; then
    (( CHECK_MODE )) && log "  [预览] 删除 $APP_SUPPORT" || { log "  → 删除 $APP_SUPPORT"; rm -rf -- "$APP_SUPPORT"; }
  fi
  for c in "${CACHES[@]}"; do
    [[ -e "$c" ]] || continue
    (( CHECK_MODE )) && log "  [预览] 删除 $c" || { log "  → 删除 $c"; rm -rf -- "$c"; }
  done
  if [[ -e "$PREFS" ]]; then
    (( CHECK_MODE )) && log "  [预览] 删除 $PREFS" || { log "  → 删除 $PREFS"; rm -rf -- "$PREFS"; }
  fi
  log "[3/4] 清理钥匙串"
  clean_keychain
  log "[4/4] 构建缓存（可选）"
  clean_build_cache

  log "== 清理完成 =="
  if (( ! CHECK_MODE )); then
    [[ -e "$APP" ]] && leftovers+=("$APP")
    [[ -e "$APP_SUPPORT" ]] && leftovers+=("$APP_SUPPORT")
    for c in "${CACHES[@]}"; do [[ -e "$c" ]] && leftovers+=("$c"); done
    [[ -e "$PREFS" ]] && leftovers+=("$PREFS")
    if (( ${#leftovers[@]} > 0 )); then
      warn "以下路径仍在: ${leftovers[*]}"
      exit 1
    fi
  fi
}

main "$@"
