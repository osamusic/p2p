#!/bin/bash

# P2P Sync ネットワーク診断スクリプト

echo "=== P2P Sync Network Diagnostics ==="
echo

# 1. プロセス確認
echo "1. Process Status:"
if pgrep -f p2p-sync > /dev/null; then
    echo "✓ p2p-sync is running"
    echo "  PID: $(pgrep -f p2p-sync)"
else
    echo "✗ p2p-sync is not running"
fi
echo

# 2. ポート確認
echo "2. Network Ports:"
echo "Listening ports:"
netstat -tlnp 2>/dev/null | grep p2p-sync || echo "No listening ports found"
echo

# 3. mDNS確認
echo "3. mDNS Discovery:"
if command -v avahi-browse >/dev/null 2>&1; then
    echo "Scanning for P2P services via mDNS..."
    timeout 5 avahi-browse -t _p2p._tcp 2>/dev/null | head -10 || echo "No mDNS services found"
elif command -v dns-sd >/dev/null 2>&1; then
    echo "Scanning for P2P services via Bonjour..."
    timeout 5 dns-sd -B _p2p._tcp 2>/dev/null | head -10 || echo "No Bonjour services found"
else
    echo "mDNS tools not available (install avahi-utils or equivalent)"
fi
echo

# 4. ファイアウォール確認
echo "4. Firewall Status:"
if command -v ufw >/dev/null 2>&1; then
    ufw status | grep -E "(4001|Status)"
elif command -v firewall-cmd >/dev/null 2>&1; then
    echo "Firewalld status: $(systemctl is-active firewalld)"
else
    echo "No common firewall tools found"
fi
echo

# 5. ログ確認
echo "5. Recent Logs (if available):"
if [ -f ~/.p2p-sync/p2p-sync.log ]; then
    echo "Last 10 log entries:"
    tail -10 ~/.p2p-sync/p2p-sync.log
elif journalctl --user -u p2p-sync --no-pager -n 5 >/dev/null 2>&1; then
    echo "systemd logs:"
    journalctl --user -u p2p-sync --no-pager -n 5
else
    echo "No log files found - check application output"
fi
echo

# 6. 設定確認
echo "6. Configuration:"
if [ -f ~/.p2p-sync/config.toml ]; then
    echo "Config file found at ~/.p2p-sync/config.toml"
    grep -E "listen_address|enable_mdns|enable_kad" ~/.p2p-sync/config.toml 2>/dev/null || echo "Default configuration"
elif [ -f /etc/p2p-sync/config.toml ]; then
    echo "System config found at /etc/p2p-sync/config.toml"
    grep -E "listen_address|enable_mdns|enable_kad" /etc/p2p-sync/config.toml 2>/dev/null
else
    echo "No config file found - using defaults"
fi
echo

echo "=== Troubleshooting Tips ==="
echo "1. Try the 'status' command in the p2p-sync shell"
echo "2. Check if port 4001 is blocked by firewall"
echo "3. Ensure mDNS is working on your network"
echo "4. Try manually connecting to another peer with:"
echo "   p2p-sync start --dial /ip4/IP_ADDRESS/tcp/4001/p2p/PEER_ID"
echo "5. Run with debug logging: RUST_LOG=debug p2p-sync start"