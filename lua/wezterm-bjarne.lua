-- ============================================================
-- 由 tools/port-cast-player-rules.py 生成 (port 自 cast-player/src/highlight.ts)
-- 33 条 keyword_highlight_rules, 每条都包 \b(?:...)\b word boundary,
-- 避免字内子串误匹配 (例如 addnetwork.sh 不会被切成 add/net/work).
--
-- 跟 cast-player 共用同一套规则 (cast-player 用 non-overlapping 算法,
-- wezterm 用内置 multi-rule 染色 — 边界对了表现接近一致).
-- ============================================================

local M = {}

-- 规则 1
table.insert(M, {
    regex = '(?i)\\b(?:no(t)?(connect)?|shutdown|shut|down|disabled|error|fail|invalid|fault|BAD|conflict|mismatch|wrong|DENY|INVALID|DISABLE|unusable|DENIED|err-disable|infinity|inaccessible|unreachable|stop|dead|blocked|forbidden|refused)\\b',
    fg = '#FC391F',
    bold = true,
})

-- 规则 2
table.insert(M, {
    regex = '\\b(?:up|enabled|active|success|running|connected|permit|establish|FULL|SYNC|OK|ESTABLISHED|forwarding|synchronized|online|healthy|started|done|completed|ready|alive|listening|accepted)\\b',
    fg = '#31E722',
    bold = true,
})

-- 规则 3
table.insert(M, {
    regex = '\\b(?:top|ps|free|df|du|iostat|vmstat|mpstat|sar|netstat|ss|lsof|fuser|strace|ltrace|pmap|slabtop|pidstat|nmon|htop|atop|iotop|mytop|powertop|perf|dstat|collectl)\\b',
    fg = '#25BC24',
    bold = true,
})

-- 规则 4
table.insert(M, {
    regex = '\\b(?:systemctl|journalctl|service|chkconfig|update-rc\\.d|init\\.d|daemon|proc|sysctl|modprobe|lsmod|insmod|rmmod|dmesg|uname|uptime|who|w|last|ac|login|sudo|su)\\b',
    fg = '#25BC24',
    bold = true,
})

-- 规则 5
table.insert(M, {
    regex = '\\b(?:ls|cd|pwd|mkdir|rm|cp|mv|touch|chmod|chown|find|grep|awk|sed|cut|sort|uniq|tr|wc|head|tail|less|more|cat|tac|nl|tee|xargs|watch|timeout|crontab|at|batch)\\b',
    fg = '#D338D3',
    bold = true,
})

-- 规则 6
table.insert(M, {
    regex = '\\b(?:tar|gzip|gunzip|bzip2|bunzip2|zip|unzip|7z|rar|unrar|rsync|scp|ssh|sftp|ftp|curl|wget|lynx|git|svn|docker|podman|kubectl|helm|ansible|puppet|salt|chef)\\b',
    fg = '#F935F8',
    bold = true,
})

-- 规则 7
table.insert(M, {
    regex = '\\b(?:ifconfig|ip|route|iptables|nft|tc|ethtool|iwconfig|nmcli|nmtui|hostnamectl|resolvectl|nslookup|dig|host|ping|traceroute|mtr|tcpdump|nmap|netcat|socat|ngrep)\\b',
    fg = '#33BBC8',
    bold = true,
})

-- 规则 8
table.insert(M, {
    regex = '\\b(?:vim|nvim|emacs|nano|joe|pico|gedit|kate|kwrite|mousepad|sublime|vscode|atom|notepad|sed|awk|perl|python|ruby|php|node|java|gcc|make|cmake|configure)\\b',
    fg = '#5833FF',
    bold = true,
})

-- 规则 9
table.insert(M, {
    regex = '\\b(?:/bin|/sbin|/etc|/dev|/proc|/sys|/var|/tmp|/usr|/opt|/root|/home|/mnt|/media|/run|/srv|\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}|[0-9a-f]{2}(?::[0-9a-f]{2}){5})\\b',
    fg = '#492EE1',
    bold = true,
})

-- 规则 10
table.insert(M, {
    regex = '\\b(?:load average|cpu[0-9]*|mem|swap|disk|net|io|system|process|thread|daemon|service|unit|target|socket|timer|mount|user|group|permission|port|protocol)\\b',
    fg = '#ADAD27',
    bold = true,
})

-- 规则 11
table.insert(M, {
    regex = '\\b(?:\\d+\\.\\d+\\.\\d+\\.\\d+(/\\d{1,2})?|\\d{1,3}%|[0-9.]+[KMGTPEZY]i?B|[0-9.]+[KMGTPEZY]Hz|\\d+:\\d+\\.\\d+|\\d{1,3}days|\\d+min|\\d+ms|\\d+us|\\d+%%|[0-9a-f]{2}(:[0-9a-f]{2}){5})\\b',
    fg = '#14F0F0',
    bold = true,
})

-- 规则 12
table.insert(M, {
    regex = '\\b(?:eth[0-9]+|wlan[0-9]+|bond[0-9]+|team[0-9]+|br[0-9]+|tun[0-9]+|tap[0-9]+|veth[0-9a-f]+|docker[0-9]+|virbr[0-9]+|vnet[0-9]+|lo|sit[0-9]+|wg[0-9]+)\\b',
    fg = '#33BBC8',
    bold = true,
})

-- 规则 13
table.insert(M, {
    regex = '\\b(?:([A-Z][a-zA-Z]*Error|Exception|Warning|Notice|Info|Debug)[: ]|\\[(ERROR|WARN|INFO|DEBUG)\\]|<(error|warning|info|debug)>|\\{"level":"(error|warn|info|debug)")\\b',
    fg = '#EAEC23',
    bold = true,
})

-- 规则 14
table.insert(M, {
    regex = '\\b(?:LISTEN|ESTABLISHED|TIME_WAIT|CLOSE_WAIT|SYN_SENT|FIN_WAIT|CLOSED|UDP|RAW|UNKNOWN|STREAM|DGRAM|SEQPACKET|PACKET|NETLINK|unix|tcp|udp|icmp|sctp)\\b',
    fg = '#818383',
    bold = true,
})

-- 规则 15
table.insert(M, {
    regex = '\\b(?:root|daemon|bin|sys|sync|games|man|lp|mail|news|uucp|proxy|www-data|backup|list|irc|gnats|nobody|systemd|messagebus|syslog|_apt|mysql|postgres|nginx|apache)\\b',
    fg = '#25BC24',
    bold = true,
})

-- 规则 16
table.insert(M, {
    regex = '\\b(?:NAME|PID|USER|PR|NI|VIRT|RES|SHR|S|%CPU|%MEM|TIME|COMMAND|VSZ|RSS|TTY|STAT|START|MAJ|MIN|B|WCHAN|PSR|TSK|P|CODE|DATA|SWAP|NSS)\\b',
    fg = '#EAEC23',
    bold = true,
})

-- 规则 17
table.insert(M, {
    regex = '\\b(?:(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\\s+\\d{1,2}\\s+\\d{2}:\\d{2}:\\d{2}|\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}(\\.\\d+)?([+-]\\d{2}:?\\d{2}|Z))\\b',
    fg = '#D338D3',
    bold = true,
})

-- 规则 18
table.insert(M, {
    regex = '\\b(?:UUID|LABEL|TYPE|DEVICE|MOUNTPOINT|OPTIONS|SIZE|USED|AVAIL|USE%|TARGET|SOURCE|FSTYPE|OWNER|GROUP|MODE|LINKS|REFERER|PROTO|STATE|MISC|POLICY)\\b',
    fg = '#ADAD27',
    bold = true,
})

-- 规则 19
table.insert(M, {
    regex = '\\b(?:\\[[0-9A-F]{8}\\]|\\([0-9A-F]{4}:[0-9A-F]{4}\\)|0x[0-9A-F]+|\\d{1,3}\\.\\d{1,3}%|[0-9.]+[KMGTPEZY]|[+-]?\\d*\\.\\d+([eE][+-]?\\d+)?)\\b',
    fg = '#ADAD27',
    bold = true,
})

-- 规则 20
table.insert(M, {
    regex = '\\b(?:CRITICAL|FATAL|ERROR|WARN|WARNING|NOTICE|INFO|DEBUG|TRACE|FINE|FINER|FINEST|ALL|OFF|EMERGENCY|ALERT|ERR|CRIT|EMERG|PANIC|NONE)\\b',
    fg = '#FC391F',
    bold = true,
})

-- 规则 21
table.insert(M, {
    regex = '\\b(?:apt|apt-get|yum|dnf|pacman|zypper|brew|snap|flatpak|pip|pip3|npm|yarn|pnpm|cargo|gem|composer|maven|gradle|conda|poetry|nix|opkg|rpm|dpkg)\\b',
    fg = '#25BC24',
    bold = true,
})

-- 规则 22
table.insert(M, {
    regex = '\\b(?:kill|killall|pkill|pgrep|nohup|disown|jobs|bg|fg|nice|renice|ionice|taskset|chrt|useradd|userdel|usermod|groupadd|groupdel|passwd|visudo|getent|whoami|groups|mkfs|mkswap|mount|umount|fdisk|parted|lsblk|blkid|findmnt|swapon|swapoff|fsck|e2fsck|tune2fs|resize2fs|xfs_repair|btrfs|zpool|zfs)\\b',
    fg = '#25BC24',
    bold = true,
})

-- 规则 23
table.insert(M, {
    regex = '\\b(?:install|uninstall|update|upgrade|remove|search|list|show|reinstall|purge|autoremove|autoclean|clean|sync|fetch|build|rebuild|publish|release|deploy|migrate|seed|rollback|backup|restore|pull|push|commit|checkout|branch|merge|rebase|fork|clone|stash|status)\\b',
    fg = '#33BBC8',
    bold = true,
})

-- 规则 24
table.insert(M, {
    regex = '\\b(?:GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS|TRACE|CONNECT|100|101|200|201|202|203|204|205|206|207|301|302|303|304|305|307|308|400|401|402|403|404|405|406|407|408|409|410|411|412|413|414|415|416|418|421|422|423|425|426|428|429|431|451|500|501|502|503|504|505|506|507|508|510|511)\\b',
    fg = '#14F0F0',
    bold = true,
})

-- 规则 25
table.insert(M, {
    regex = '\\b(?:HTTP|HTTPS|HTTP/1\\.0|HTTP/1\\.1|HTTP/2|HTTP/3|FTP|FTPS|SFTP|TFTP|TELNET|SSH|SCP|SMTP|SMTPS|POP3|POP3S|IMAP|IMAPS|DNS|DHCP|NTP|SNMP|LDAP|LDAPS|RDP|VNC|NFS|SMB|CIFS|iSCSI|gRPC|WebSocket|WS|WSS|MQTT|AMQP|STOMP|XMPP|IRC|RTSP|RTP|RTCP|SIP|TLS|SSL|mTLS|QUIC|BGP|OSPF|RIP|VRRP|HSRP|GRE|VXLAN|MPLS|IPSec|IKE|OpenVPN|WireGuard)\\b',
    fg = '#33BBC8',
    bold = true,
})

-- 规则 26
table.insert(M, {
    regex = '\\b(?:if|then|elif|else|fi|case|esac|for|while|until|do|done|break|continue|return|exit|select|function|in|time|export|source|alias|unalias|local|readonly|declare|typeset|let|set|unset|shift|getopts|trap|exec|eval|wait|read)\\b',
    fg = '#EAEC23',
    bold = true,
})

-- 规则 27
table.insert(M, {
    regex = '\\b(?:mariadb|oracle|mongo|mongodb|redis|memcached|cassandra|elasticsearch|elastic|kibana|logstash|grafana|prometheus|alertmanager|node_exporter|influxdb|clickhouse|cockroachdb|cockroach|tidb|tikv|etcd|consul|vault|zookeeper|sqlite|kafka|pulsar|rabbitmq|nats|nsq|haproxy|envoy|traefik|caddy|apache2|httpd|tomcat|jetty|wildfly|nodejs|gunicorn|uwsgi|supervisord|cron|crond|fail2ban|firewalld|sshguard|containerd|cri-o|podman|kubernetes|k3s|k8s|kubelet|kube-proxy|kube-apiserver|kube-controller|kube-scheduler|helm|istio|linkerd|cilium|flannel|calico|weave|metallb|argocd|tekton|spinnaker|jenkins|gitlab|gitea|harbor)\\b',
    fg = '#33BBC8',
    bold = true,
})

-- 规则 28
table.insert(M, {
    regex = '\\b(?:Pod|Pods|Deployment|Deployments|StatefulSet|StatefulSets|DaemonSet|DaemonSets|ReplicaSet|ReplicaSets|ReplicationController|Service|Services|Ingress|Ingresses|Namespace|Namespaces|ConfigMap|ConfigMaps|Secret|Secrets|PersistentVolume|PersistentVolumes|PersistentVolumeClaim|PersistentVolumeClaims|PVC|PV|Node|Nodes|Job|Jobs|CronJob|CronJobs|HPA|HorizontalPodAutoscaler|VPA|VerticalPodAutoscaler|Role|Roles|RoleBinding|RoleBindings|ClusterRole|ClusterRoles|ClusterRoleBinding|ClusterRoleBindings|ServiceAccount|ServiceAccounts|NetworkPolicy|NetworkPolicies|StorageClass|StorageClasses|CustomResourceDefinition|CRD|CRDs|Endpoint|Endpoints|EndpointSlice|EndpointSlices|Event|Events|Lease|MutatingWebhookConfiguration|ValidatingWebhookConfiguration)\\b',
    fg = '#D338D3',
    bold = true,
})

-- 规则 29
table.insert(M, {
    regex = '\\b(?:timeout|timed[- ]out|refused|reset|closed|aborted|cancelled|canceled|dropped|expired|revoked|banned|locked|unlocked|reconnected|reconnecting|retry|retrying|retried|throttled|rate[- ]limited|denied|granted|allowed|blocked|handshake|negotiated|renegotiated|disconnected|disconnect|connecting|accepting|accepted|rejecting|rejected)\\b',
    fg = '#F935F8',
    bold = true,
})

-- 规则 30
table.insert(M, {
    regex = '\\b(?:panic|abort|aborted|segfault|segmentation fault|oom|oom-killed|oomkilled|oom_killer|coredump|core dumped|sigterm|sigkill|sigint|sigsegv|sigabrt|sigfpe|sigbus|sigpipe|sighup|sigchld|sigusr[12]|stack overflow|null pointer|deadlock|race condition|double free|use after free|memory leak|fd leak|file descriptor leak|out of memory|too many open files)\\b',
    fg = '#FC391F',
    bold = true,
})

-- 规则 31
table.insert(M, {
    regex = '\\b(?:true|false|null|nil|None|NULL|undefined|NaN|Infinity|Some|Ok|Err|TRUE|FALSE|void|new|delete)\\b',
    fg = '#D338D3',
    bold = true,
})

-- 规则 32
table.insert(M, {
    regex = '\\b(?:SELECT|FROM|WHERE|AND|OR|NOT|INSERT|INTO|VALUES|UPDATE|SET|DELETE|JOIN|INNER|LEFT|RIGHT|OUTER|FULL|CROSS|GROUP|ORDER|BY|HAVING|LIMIT|OFFSET|CREATE|DROP|ALTER|TABLE|INDEX|VIEW|UNIQUE|PRIMARY|KEY|FOREIGN|REFERENCES|CASCADE|DEFAULT|EXPLAIN|ANALYZE|VACUUM|TRANSACTION|BEGIN|COMMIT|ROLLBACK|SAVEPOINT|TRIGGER|PROCEDURE|FUNCTION|DATABASE|SCHEMA|GRANT|REVOKE|UNION|ALL|DISTINCT|AS|ON|IS|EXISTS|BETWEEN|LIKE|ILIKE|CASE|WHEN|THEN|ELSE|END|COUNT|SUM|AVG|MAX|MIN|COALESCE|NULLIF|CAST|EXTRACT|REGEXP|MATCH|AGAINST|RETURNING|WITH|RECURSIVE|WINDOW|PARTITION|OVER)\\b',
    fg = '#ADAD27',
    bold = true,
})

-- 规则 33
table.insert(M, {
    regex = '\\b(?:fn|func|def|class|let|mut|pub|use|mod|impl|trait|where|dyn|async|await|move|match|Result|Option|Vec|String|Box|Arc|Rc|Mutex|RwLock|Self|Sized|derive|crate|extern|ref|loop|var|const|type|struct|interface|package|import|export|defer|chan|range|iota|make|map|nil|error|from|return|yield|lambda|with|try|except|finally|raise|pass|self|cls|global|nonlocal|elif|throw|throws|catch|instanceof|typeof|prototype|this|super|abstract|public|private|protected|final|static|virtual|override|implements|extends|enum|template|namespace|inline|volatile|register|sizeof|nullptr|auto)\\b',
    fg = '#D338D3',
    bold = true,
})

return M
