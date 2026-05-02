-- ============================================================
-- 由 tools/secure-crt-to-wezterm-rules.py 自动生成
-- 来源:/Users/bjarne/Work/安全工具/SecureCRT/9.6/KnownHosts/roy.ini
-- 列表名称:roy
-- Match Case(大小写敏感):True
-- 启用规则数:20
-- 总规则数:20
-- ============================================================
--
-- 用法(在 ~/.wezterm.lua 中):
--
--   local wezterm = require 'wezterm'
--   local config = wezterm.config_builder()
--   config.keyword_highlight_rules = require 'wezterm-roy'
--   return config
--
-- 该文件需要 fork 版 WezTerm(支持 keyword_highlight_rules)。
-- 标准 WezTerm 不识别此配置,会在加载时报错。
-- ============================================================

local M = {}

-- 规则 1: (no(t)?(connect)?)|((shut)?(down)?)|disabled|error|fail|invalid|fau...
table.insert(M, {
    regex = '(no(t)?(connect)?)|((shut)?(down)?)|disabled|error|fail|invalid|fault|BAD|conflict|mismatch|wrong|DENY|INVALID|DISABLE|unusable|DENIED|err-disable|infinity|inaccessible|unreachable|stop|dead|blocked|forbidden|refused)',
    fg = '#FF0000',
    bold = true,
})

-- 规则 2: (up|enabled|active|success|running|connected|permit|establish|FULL|...
table.insert(M, {
    regex = '(up|enabled|active|success|running|connected|permit|establish|FULL|SYNC|OK|ESTABLISHED|forwarding|synchronized|online|healthy|started|done|completed|ready|alive|listening|accepted)',
    fg = '#00FF00',
    bold = true,
})

-- 规则 3: (top|ps|free|df|du|iostat|vmstat|mpstat|sar|netstat|ss|lsof|fuser|s...
table.insert(M, {
    regex = '(top|ps|free|df|du|iostat|vmstat|mpstat|sar|netstat|ss|lsof|fuser|strace|ltrace|pmap|slabtop|pidstat|nmon|htop|atop|iotop|mytop|powertop|perf|dstat|collectl)',
    fg = '#0080FF',
    bold = true,
})

-- 规则 4: (systemctl|journalctl|service|chkconfig|update-rc\.d|init\.d|daemon...
table.insert(M, {
    regex = '(systemctl|journalctl|service|chkconfig|update-rc\\.d|init\\.d|daemon|proc|sysctl|modprobe|lsmod|insmod|rmmod|dmesg|uname|uptime|who|w|last|ac|login|sudo|su)',
    fg = '#0080FF',
    bold = true,
})

-- 规则 5: (ls|cd|pwd|mkdir|rm|cp|mv|touch|chmod|chown|find|grep|awk|sed|cut|s...
table.insert(M, {
    regex = '(ls|cd|pwd|mkdir|rm|cp|mv|touch|chmod|chown|find|grep|awk|sed|cut|sort|uniq|tr|wc|head|tail|less|more|cat|tac|nl|tee|xargs|watch|timeout|crontab|at|batch)',
    fg = '#FF00FF',
    bold = true,
})

-- 规则 6: (tar|gzip|gunzip|bzip2|bunzip2|zip|unzip|7z|rar|unrar|rsync|scp|ssh...
table.insert(M, {
    regex = '(tar|gzip|gunzip|bzip2|bunzip2|zip|unzip|7z|rar|unrar|rsync|scp|ssh|sftp|ftp|curl|wget|lynx|git|svn|docker|podman|kubectl|helm|ansible|puppet|salt|chef)',
    fg = '#FF00FF',
    bold = true,
})

-- 规则 7: (ifconfig|ip|route|iptables|nft|tc|ethtool|iwconfig|nmcli|nmtui|hos...
table.insert(M, {
    regex = '(ifconfig|ip|route|iptables|nft|tc|ethtool|iwconfig|nmcli|nmtui|hostnamectl|resolvectl|nslookup|dig|host|ping|traceroute|mtr|tcpdump|nmap|netcat|socat|ngrep)',
    fg = '#FF00FF',
    bold = true,
})

-- 规则 8: (vim|nvim|emacs|nano|joe|pico|gedit|kate|kwrite|mousepad|sublime|vs...
table.insert(M, {
    regex = '(vim|nvim|emacs|nano|joe|pico|gedit|kate|kwrite|mousepad|sublime|vscode|atom|notepad|sed|awk|perl|python|ruby|php|node|java|gcc|make|cmake|configure)',
    fg = '#FFFF00',
    bold = true,
})

-- 规则 9: (/bin|/sbin|/etc|/dev|/proc|/sys|/var|/tmp|/usr|/opt|/root|/home|/m...
table.insert(M, {
    regex = '(/bin|/sbin|/etc|/dev|/proc|/sys|/var|/tmp|/usr|/opt|/root|/home|/mnt|/media|/run|/srv|\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}|[0-9a-f:]{2,})',
    fg = '#FFFF00',
    bold = true,
})

-- 规则 10: (load average|cpu[0-9]*|mem|swap|disk|net|io|system|process|thread|...
table.insert(M, {
    regex = '(load average|cpu[0-9]*|mem|swap|disk|net|io|system|process|thread|daemon|service|unit|target|socket|timer|mount|user|group|permission|port|protocol)',
    fg = '#FF00FF',
    bold = true,
})

-- 规则 11: (\d+\.\d+\.\d+\.\d+(/\d{1,2})?|\d{1,3}%|[0-9.]+[KMGTPEZY]i?B|[0-9.]...
table.insert(M, {
    regex = '(\\d+\\.\\d+\\.\\d+\\.\\d+(/\\d{1,2})?|\\d{1,3}%|[0-9.]+[KMGTPEZY]i?B|[0-9.]+[KMGTPEZY]Hz|\\d+:\\d+\\.\\d+|\\d{1,3}days|\\d+min|\\d+ms|\\d+us|\\d+%%|[0-9a-f]{2}(:[0-9a-f]{2}){5})',
    fg = '#FF8000',
    bold = true,
})

-- 规则 12: (eth[0-9]+|wlan[0-9]+|bond[0-9]+|team[0-9]+|br[0-9]+|tun[0-9]+|tap[...
table.insert(M, {
    regex = '(eth[0-9]+|wlan[0-9]+|bond[0-9]+|team[0-9]+|br[0-9]+|tun[0-9]+|tap[0-9]+|veth[0-9a-f]+|docker[0-9]+|virbr[0-9]+|vnet[0-9]+|lo|sit[0-9]+|wg[0-9]+)',
    fg = '#FF8000',
    bold = true,
})

-- 规则 13: (([A-Z][a-zA-Z]*Error|Exception|Warning|Notice|Info|Debug)[: ]|\[(E...
table.insert(M, {
    regex = '(([A-Z][a-zA-Z]*Error|Exception|Warning|Notice|Info|Debug)[: ]|\\[(ERROR|WARN|INFO|DEBUG)\\]|<(error|warning|info|debug)>|\\{\\"level\\":\\"(error|warn|info|debug)\\")',
    fg = '#FFFF75',
    bold = true,
})

-- 规则 14: (LISTEN|ESTABLISHED|TIME_WAIT|CLOSE_WAIT|SYN_SENT|FIN_WAIT|CLOSED|U...
table.insert(M, {
    regex = '(LISTEN|ESTABLISHED|TIME_WAIT|CLOSE_WAIT|SYN_SENT|FIN_WAIT|CLOSED|UDP|RAW|UNKNOWN|STREAM|DGRAM|SEQPACKET|PACKET|NETLINK|unix|tcp|udp|icmp|sctp)',
    fg = '#CECECE',
    bold = true,
})

-- 规则 15: (root|daemon|bin|sys|sync|games|man|lp|mail|news|uucp|proxy|www-dat...
table.insert(M, {
    regex = '(root|daemon|bin|sys|sync|games|man|lp|mail|news|uucp|proxy|www-data|backup|list|irc|gnats|nobody|systemd|messagebus|syslog|_apt|mysql|postgres|nginx|apache)',
    fg = '#FF00FF',
    bold = true,
})

-- 规则 16: (NAME|PID|USER|PR|NI|VIRT|RES|SHR|S|%CPU|%MEM|TIME|COMMAND|VSZ|RSS|...
table.insert(M, {
    regex = '(NAME|PID|USER|PR|NI|VIRT|RES|SHR|S|%CPU|%MEM|TIME|COMMAND|VSZ|RSS|TTY|STAT|START|TIME|MAJ|MIN|B|WCHAN|PSR|TSK|P|CODE|DATA|SWAP|NSS)',
    fg = '#FFD700',
    bold = true,
})

-- 规则 17: (Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d{1,2}\s+\d{2}...
table.insert(M, {
    regex = '(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\\s+\\d{1,2}\\s+\\d{2}:\\d{2}:\\d{2}|\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}(\\.\\d+)?([+-]\\d{2}:?\\d{2}|Z)',
    fg = '#AA00AA',
    bold = true,
})

-- 规则 18: (UUID|LABEL|TYPE|DEVICE|MOUNTPOINT|OPTIONS|SIZE|USED|AVAIL|USE%|TAR...
table.insert(M, {
    regex = '(UUID|LABEL|TYPE|DEVICE|MOUNTPOINT|OPTIONS|SIZE|USED|AVAIL|USE%|TARGET|SOURCE|FSTYPE|OWNER|GROUP|MODE|LINKS|REFERER|PROTO|STATE|MISC|POLICY)',
    fg = '#CC66FF',
    bold = true,
})

-- 规则 19: (\[[0-9A-F]{8}\]|\([0-9A-F]{4}:[0-9A-F]{4}\)|0x[0-9A-F]+|\d{1,3}\.\...
table.insert(M, {
    regex = '(\\[[0-9A-F]{8}\\]|\\([0-9A-F]{4}:[0-9A-F]{4}\\)|0x[0-9A-F]+|\\d{1,3}\\.\\d{1,3}%|[0-9.]+[KMGTPEZY]|[+-]?\\d*\\.\\d+([eE][+-]?\\d+)?)',
    fg = '#99CC33',
    bold = true,
})

-- 规则 20: (CRITICAL|FATAL|ERROR|WARN|WARNING|NOTICE|INFO|DEBUG|TRACE|FINE|FIN...
table.insert(M, {
    regex = '(CRITICAL|FATAL|ERROR|WARN|WARNING|NOTICE|INFO|DEBUG|TRACE|FINE|FINER|FINEST|ALL|OFF|EMERGENCY|ALERT|ERR|CRIT|EMERG|PANIC|NONE)',
    fg = '#FF55AA',
    bold = true,
})

-- ============================================================
-- L2 补充候选规则(默认注释,按需启用)
-- 来源:zsh-syntax-highlighting + 常见输出场景盲区
-- ============================================================

-- L2-1 双引号字符串(JSON / 配置文件常见)
-- table.insert(M, { regex = [["[^"\\n]*"]], fg = '#FFD700' })

-- L2-2 单引号字符串
-- table.insert(M, { regex = [[\'[^\'\\n]*\']], fg = '#FFD700' })

-- L2-3 shell 变量($PATH / ${HOME})
-- table.insert(M, { regex = [[\$\{?\w+\}?]], fg = '#C586C0' })

-- L2-5 git / docker hash(7-40 位 hex)
-- table.insert(M, { regex = [[\b[0-9a-f]{7,40}\b]], fg = '#888888' })

-- L2-6 文件路径(带扩展名)
-- table.insert(M, {
--     regex = [[[\w./-]+\.(log|conf|ya?ml|json|sh|py|rs|go|md)\b]],
--     fg = '#00FFFF',
-- })

return M
