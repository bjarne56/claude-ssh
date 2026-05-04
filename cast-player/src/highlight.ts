// 关键字高亮 — 移植自 lua/wezterm-roy.lua 的 33 条规则
// 对纯文本匹配关键字, 包 ANSI 24-bit truecolor 序列再写回 xterm.
// 支持开关, 关闭时直接返回原文本.

interface Rule {
  re: RegExp;
  fg: string; // hex like '#FC391F'
  bold?: boolean;
}

// hex → ANSI truecolor (38;2;R;G;B)
function hexToAnsi(hex: string, bold = false): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return bold ? `\x1b[1;38;2;${r};${g};${b}m` : `\x1b[38;2;${r};${g};${b}m`;
}
const RESET = "\x1b[0m";

// 全部规则 — 跟 wezterm-roy.lua 字面对应
// JS regex 注意: \b 兼容, lua \\b 转 \b; (?:...) 兼容; \\d 转 \d
const RULES: Rule[] = [
  // 1. 错误/失败类 (红)
  { re: /\b(?:no(t)?(connect)?|shutdown|shut|down|disabled|error|fail|invalid|fault|BAD|conflict|mismatch|wrong|DENY|INVALID|DISABLE|unusable|DENIED|err-disable|infinity|inaccessible|unreachable|stop|dead|blocked|forbidden|refused)\b/gi, fg: "#FC391F", bold: true },
  // 2. 成功/up 类 (亮绿)
  { re: /\b(?:up|enabled|active|success|running|connected|permit|establish|FULL|SYNC|OK|ESTABLISHED|forwarding|synchronized|online|healthy|started|done|completed|ready|alive|listening|accepted)\b/g, fg: "#31E722", bold: true },
  // 3. 系统/性能命令 (绿)
  { re: /\b(?:top|ps|free|df|du|iostat|vmstat|mpstat|sar|netstat|ss|lsof|fuser|strace|ltrace|pmap|slabtop|pidstat|nmon|htop|atop|iotop|mytop|powertop|perf|dstat|collectl)\b/g, fg: "#25BC24", bold: true },
  // 4. 系统服务命令 (绿)
  { re: /\b(?:systemctl|journalctl|service|chkconfig|update-rc\.d|init\.d|daemon|proc|sysctl|modprobe|lsmod|insmod|rmmod|dmesg|uname|uptime|who|w|last|ac|login|sudo|su)\b/g, fg: "#25BC24", bold: true },
  // 5. 文件操作命令 (洋红)
  { re: /\b(?:ls|cd|pwd|mkdir|rm|cp|mv|touch|chmod|chown|find|grep|awk|sed|cut|sort|uniq|tr|wc|head|tail|less|more|cat|tac|nl|tee|xargs|watch|timeout|crontab|at|batch)\b/g, fg: "#D338D3", bold: true },
  // 6. 网络/传输命令 (亮洋红)
  { re: /\b(?:tar|gzip|gunzip|bzip2|bunzip2|zip|unzip|7z|rar|unrar|rsync|scp|ssh|sftp|ftp|curl|wget|lynx|git|svn|docker|podman|kubectl|helm|ansible|puppet|salt|chef)\b/g, fg: "#F935F8", bold: true },
  // 7. 网络配置命令 (青)
  { re: /\b(?:ifconfig|ip|route|iptables|nft|tc|ethtool|iwconfig|nmcli|nmtui|hostnamectl|resolvectl|nslookup|dig|host|ping|traceroute|mtr|tcpdump|nmap|netcat|socat|ngrep)\b/g, fg: "#33BBC8", bold: true },
  // 8. 编辑器/编译 (蓝)
  { re: /\b(?:vim|nvim|emacs|nano|joe|pico|gedit|kate|kwrite|mousepad|sublime|vscode|atom|notepad|sed|awk|perl|python|ruby|php|node|java|gcc|make|cmake|configure)\b/g, fg: "#5833FF", bold: true },
  // 9. 系统路径 + IP/MAC (蓝)
  { re: /\b(?:\/bin|\/sbin|\/etc|\/dev|\/proc|\/sys|\/var|\/tmp|\/usr|\/opt|\/root|\/home|\/mnt|\/media|\/run|\/srv|\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}|[0-9a-f]{2}(?::[0-9a-f]{2}){5})\b/g, fg: "#492EE1", bold: true },
  // 10. 系统资源类 (淡黄)
  { re: /\b(?:load average|cpu[0-9]*|mem|swap|disk|net|io|system|process|thread|daemon|service|unit|target|socket|timer|mount|user|group|permission|port|protocol)\b/g, fg: "#ADAD27", bold: true },
  // 11. 数值单位 (亮青)
  { re: /\b(?:\d+\.\d+\.\d+\.\d+(\/\d{1,2})?|\d{1,3}%|[0-9.]+[KMGTPEZY]i?B|[0-9.]+[KMGTPEZY]Hz|\d+:\d+\.\d+|\d{1,3}days|\d+min|\d+ms|\d+us|\d+%%|[0-9a-f]{2}(:[0-9a-f]{2}){5})\b/g, fg: "#14F0F0", bold: true },
  // 12. 网络接口名 (青)
  { re: /\b(?:eth[0-9]+|wlan[0-9]+|bond[0-9]+|team[0-9]+|br[0-9]+|tun[0-9]+|tap[0-9]+|veth[0-9a-f]+|docker[0-9]+|virbr[0-9]+|vnet[0-9]+|lo|sit[0-9]+|wg[0-9]+)\b/g, fg: "#33BBC8", bold: true },
  // 13. 错误日志 prefix (亮黄)
  { re: /\b(?:([A-Z][a-zA-Z]*Error|Exception|Warning|Notice|Info|Debug)[: ]|\[(ERROR|WARN|INFO|DEBUG)\]|<(error|warning|info|debug)>|\{"level":"(error|warn|info|debug)")\b/g, fg: "#EAEC23", bold: true },
  // 14. 网络连接状态 (灰)
  { re: /\b(?:LISTEN|ESTABLISHED|TIME_WAIT|CLOSE_WAIT|SYN_SENT|FIN_WAIT|CLOSED|UDP|RAW|UNKNOWN|STREAM|DGRAM|SEQPACKET|PACKET|NETLINK|unix|tcp|udp|icmp|sctp)\b/g, fg: "#818383", bold: true },
  // 15. 系统用户名 (绿)
  { re: /\b(?:root|daemon|bin|sys|sync|games|man|lp|mail|news|uucp|proxy|www-data|backup|list|irc|gnats|nobody|systemd|messagebus|syslog|_apt|mysql|postgres|nginx|apache)\b/g, fg: "#25BC24", bold: true },
  // 16. ps/top 表头 (亮黄)
  { re: /\b(?:NAME|PID|USER|PR|NI|VIRT|RES|SHR|S|%CPU|%MEM|TIME|COMMAND|VSZ|RSS|TTY|STAT|START|MAJ|MIN|B|WCHAN|PSR|TSK|P|CODE|DATA|SWAP|NSS)\b/g, fg: "#EAEC23", bold: true },
  // 17. 时间戳 (洋红)
  { re: /\b(?:(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}|\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?([+-]\d{2}:?\d{2}|Z))\b/g, fg: "#D338D3", bold: true },
  // 18. 文件系统标签 (淡黄)
  { re: /\b(?:UUID|LABEL|TYPE|DEVICE|MOUNTPOINT|OPTIONS|SIZE|USED|AVAIL|USE%|TARGET|SOURCE|FSTYPE|OWNER|GROUP|MODE|LINKS|REFERER|PROTO|STATE|MISC|POLICY)\b/g, fg: "#ADAD27", bold: true },
  // 19. hex / 浮点数 (淡黄)
  { re: /\b(?:\[[0-9A-F]{8}\]|\([0-9A-F]{4}:[0-9A-F]{4}\)|0x[0-9A-F]+|\d{1,3}\.\d{1,3}%|[0-9.]+[KMGTPEZY]|[+-]?\d*\.\d+([eE][+-]?\d+)?)\b/g, fg: "#ADAD27", bold: true },
  // 20. 严重错误级别 (红)
  { re: /\b(?:CRITICAL|FATAL|ERROR|WARN|WARNING|NOTICE|INFO|DEBUG|TRACE|FINE|FINER|FINEST|ALL|OFF|EMERGENCY|ALERT|ERR|CRIT|EMERG|PANIC|NONE)\b/g, fg: "#FC391F", bold: true },
  // 21. 包管理器 (绿)
  { re: /\b(?:apt|apt-get|yum|dnf|pacman|zypper|brew|snap|flatpak|pip|pip3|npm|yarn|pnpm|cargo|gem|composer|maven|gradle|conda|poetry|nix|opkg|rpm|dpkg)\b/g, fg: "#25BC24", bold: true },
  // 22. 进程/用户管理 (绿)
  { re: /\b(?:kill|killall|pkill|pgrep|nohup|disown|jobs|bg|fg|nice|renice|ionice|taskset|chrt|useradd|userdel|usermod|groupadd|groupdel|passwd|visudo|getent|whoami|groups|mkfs|mkswap|mount|umount|fdisk|parted|lsblk|blkid|findmnt|swapon|swapoff|fsck|e2fsck|tune2fs|resize2fs|xfs_repair|btrfs|zpool|zfs)\b/g, fg: "#25BC24", bold: true },
  // 23. 包管理动作 (青)
  { re: /\b(?:install|uninstall|update|upgrade|remove|search|list|show|reinstall|purge|autoremove|autoclean|clean|sync|fetch|build|rebuild|publish|release|deploy|migrate|seed|rollback|backup|restore|pull|push|commit|checkout|branch|merge|rebase|fork|clone|stash|status)\b/g, fg: "#33BBC8", bold: true },
  // 24. HTTP 方法 + 状态码 (亮青)
  { re: /\b(?:GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS|TRACE|CONNECT|100|101|200|201|202|203|204|205|206|207|301|302|303|304|305|307|308|400|401|402|403|404|405|406|407|408|409|410|411|412|413|414|415|416|418|421|422|423|425|426|428|429|431|451|500|501|502|503|504|505|506|507|508|510|511)\b/g, fg: "#14F0F0", bold: true },
  // 25. 网络协议 (青)
  { re: /\b(?:HTTP|HTTPS|HTTP\/1\.0|HTTP\/1\.1|HTTP\/2|HTTP\/3|FTP|FTPS|SFTP|TFTP|TELNET|SSH|SCP|SMTP|SMTPS|POP3|POP3S|IMAP|IMAPS|DNS|DHCP|NTP|SNMP|LDAP|LDAPS|RDP|VNC|NFS|SMB|CIFS|iSCSI|gRPC|WebSocket|WS|WSS|MQTT|AMQP|STOMP|XMPP|IRC|RTSP|RTP|RTCP|SIP|TLS|SSL|mTLS|QUIC|BGP|OSPF|RIP|VRRP|HSRP|GRE|VXLAN|MPLS|IPSec|IKE|OpenVPN|WireGuard)\b/g, fg: "#33BBC8", bold: true },
  // 26. shell 关键字 (亮黄)
  { re: /\b(?:if|then|elif|else|fi|case|esac|for|while|until|do|done|break|continue|return|exit|select|function|in|time|export|source|alias|unalias|local|readonly|declare|typeset|let|set|unset|shift|getopts|trap|exec|eval|wait|read)\b/g, fg: "#EAEC23", bold: true },
  // 27. 数据库/中间件 (青)
  { re: /\b(?:mariadb|oracle|mongo|mongodb|redis|memcached|cassandra|elasticsearch|elastic|kibana|logstash|grafana|prometheus|alertmanager|node_exporter|influxdb|clickhouse|cockroachdb|cockroach|tidb|tikv|etcd|consul|vault|zookeeper|sqlite|kafka|pulsar|rabbitmq|nats|nsq|haproxy|envoy|traefik|caddy|apache2|httpd|tomcat|jetty|wildfly|nodejs|gunicorn|uwsgi|supervisord|cron|crond|fail2ban|firewalld|sshguard|containerd|cri-o|podman|kubernetes|k3s|k8s|kubelet|kube-proxy|kube-apiserver|kube-controller|kube-scheduler|helm|istio|linkerd|cilium|flannel|calico|weave|metallb|argocd|tekton|spinnaker|jenkins|gitlab|gitea|harbor)\b/g, fg: "#33BBC8", bold: true },
  // 28. K8s 资源 (洋红)
  { re: /\b(?:Pod|Pods|Deployment|Deployments|StatefulSet|StatefulSets|DaemonSet|DaemonSets|ReplicaSet|ReplicaSets|ReplicationController|Service|Services|Ingress|Ingresses|Namespace|Namespaces|ConfigMap|ConfigMaps|Secret|Secrets|PersistentVolume|PersistentVolumes|PersistentVolumeClaim|PersistentVolumeClaims|PVC|PV|Node|Nodes|Job|Jobs|CronJob|CronJobs|HPA|HorizontalPodAutoscaler|VPA|VerticalPodAutoscaler|Role|Roles|RoleBinding|RoleBindings|ClusterRole|ClusterRoles|ClusterRoleBinding|ClusterRoleBindings|ServiceAccount|ServiceAccounts|NetworkPolicy|NetworkPolicies|StorageClass|StorageClasses|CustomResourceDefinition|CRD|CRDs|Endpoint|Endpoints|EndpointSlice|EndpointSlices|Event|Events|Lease|MutatingWebhookConfiguration|ValidatingWebhookConfiguration)\b/g, fg: "#D338D3", bold: true },
  // 29. 网络异常状态 (亮洋红)
  { re: /\b(?:timeout|timed[- ]out|refused|reset|closed|aborted|cancelled|canceled|dropped|expired|revoked|banned|locked|unlocked|reconnected|reconnecting|retry|retrying|retried|throttled|rate[- ]limited|denied|granted|allowed|blocked|handshake|negotiated|renegotiated|disconnected|disconnect|connecting|accepting|accepted|rejecting|rejected)\b/g, fg: "#F935F8", bold: true },
  // 30. 严重错误关键字 (红)
  { re: /\b(?:panic|abort|aborted|segfault|segmentation fault|oom|oom-killed|oomkilled|oom_killer|coredump|core dumped|sigterm|sigkill|sigint|sigsegv|sigabrt|sigfpe|sigbus|sigpipe|sighup|sigchld|sigusr[12]|stack overflow|null pointer|deadlock|race condition|double free|use after free|memory leak|fd leak|file descriptor leak|out of memory|too many open files)\b/g, fg: "#FC391F", bold: true },
  // 31. 布尔/null (洋红)
  { re: /\b(?:true|false|null|nil|None|NULL|undefined|NaN|Infinity|Some|Ok|Err|TRUE|FALSE|void|new|delete)\b/g, fg: "#D338D3", bold: true },
  // 32. SQL 关键字 (淡黄)
  { re: /\b(?:SELECT|FROM|WHERE|AND|OR|NOT|INSERT|INTO|VALUES|UPDATE|SET|DELETE|JOIN|INNER|LEFT|RIGHT|OUTER|FULL|CROSS|GROUP|ORDER|BY|HAVING|LIMIT|OFFSET|CREATE|DROP|ALTER|TABLE|INDEX|VIEW|UNIQUE|PRIMARY|KEY|FOREIGN|REFERENCES|CASCADE|DEFAULT|EXPLAIN|ANALYZE|VACUUM|TRANSACTION|BEGIN|COMMIT|ROLLBACK|SAVEPOINT|TRIGGER|PROCEDURE|FUNCTION|DATABASE|SCHEMA|GRANT|REVOKE|UNION|ALL|DISTINCT|AS|ON|IS|EXISTS|BETWEEN|LIKE|ILIKE|CASE|WHEN|THEN|ELSE|END|COUNT|SUM|AVG|MAX|MIN|COALESCE|NULLIF|CAST|EXTRACT|REGEXP|MATCH|AGAINST|RETURNING|WITH|RECURSIVE|WINDOW|PARTITION|OVER)\b/g, fg: "#ADAD27", bold: true },
  // 33. 编程语言关键字 (洋红)
  { re: /\b(?:fn|func|def|class|let|mut|pub|use|mod|impl|trait|where|dyn|async|await|move|match|Result|Option|Vec|String|Box|Arc|Rc|Mutex|RwLock|Self|Sized|derive|crate|extern|ref|loop|var|const|type|struct|interface|package|import|export|defer|chan|range|iota|make|map|nil|error|from|return|yield|lambda|with|try|except|finally|raise|pass|self|cls|global|nonlocal|elif|throw|throws|catch|instanceof|typeof|prototype|this|super|abstract|public|private|protected|final|static|virtual|override|implements|extends|enum|template|namespace|inline|volatile|register|sizeof|nullptr|auto)\b/g, fg: "#D338D3", bold: true },
];

// ssh-ops marker 协议遗留物: BEGIN_<nonce> / END_<nonce>=N
// player 回放时过滤掉, 不显示给用户 (cast 原文件保留可审计)
const MARKER_RE = /SSHOPS_(?:BEGIN|END)_[0-9a-f]{16}(?:=-?\d+)?\r?\n?/g;

/**
 * 去除 ssh-ops marker 行 (回放时不显示)
 */
export function stripMarkers(text: string): string {
  return text.replace(MARKER_RE, "");
}

// 跨 chunk 累积不完整行 (单例 buffer)
let lineBuf = "";

/**
 * 把 PTY 输出文本中的关键字包 ANSI truecolor.
 * 流式输入: 不完整行(无 \n)缓存到下一次, 避免跨 chunk 错位匹配.
 *
 * @param text 新到达的字节文本 (含 ANSI / \r / \n / 任意控制序列)
 * @returns 加了高亮 ANSI 的文本 (ready 写到 xterm)
 */
export function applyHighlight(text: string): string {
  lineBuf += text;
  // 末尾可能不完整一行, 保留下来
  const lastNl = lineBuf.lastIndexOf("\n");
  if (lastNl < 0) {
    // 整段无换行 → 全部缓存等下一 chunk
    return "";
  }
  const ready = lineBuf.slice(0, lastNl + 1);
  lineBuf = lineBuf.slice(lastNl + 1);
  return highlightOnce(ready);
}

/**
 * 对一段文本一次性应用所有规则, 不嵌套不破坏.
 *
 * 算法:
 * 1. 每条规则独立 match 整段, 收集所有 (start, end, fg, bold, ruleIdx)
 * 2. 按 start 升序; tie-break: 更长 match wins; 再 tie: 先定义的规则 wins
 * 3. 重叠时跳过后到的 (greedy 第一个)
 * 4. 单次重建: 按 ranges 切片, 包 ANSI
 */
function highlightOnce(text: string): string {
  type M = { start: number; end: number; fg: string; bold?: boolean; ri: number };
  const matches: M[] = [];
  for (let ri = 0; ri < RULES.length; ri++) {
    const rule = RULES[ri];
    rule.re.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = rule.re.exec(text)) !== null) {
      if (m[0].length === 0) {
        rule.re.lastIndex++;
        continue;
      }
      matches.push({ start: m.index, end: m.index + m[0].length, fg: rule.fg, bold: rule.bold, ri });
    }
  }
  // 排序: 先 start, 先定义的规则优先 (跟 wezterm keyword_highlight_rules 行为一致),
  // 同 ri 取更长 match
  matches.sort((a, b) =>
    a.start - b.start ||
    a.ri - b.ri ||
    (b.end - b.start) - (a.end - a.start)
  );
  // 去重叠: 同位置只取第一个, 后面跳过
  const filtered: M[] = [];
  let lastEnd = 0;
  for (const m of matches) {
    if (m.start >= lastEnd) {
      filtered.push(m);
      lastEnd = m.end;
    }
  }
  // 拼接
  let out = "";
  let pos = 0;
  for (const m of filtered) {
    out += text.slice(pos, m.start);
    out += hexToAnsi(m.fg, m.bold) + text.slice(m.start, m.end) + RESET;
    pos = m.end;
  }
  out += text.slice(pos);
  return out;
}

/**
 * seek/reset 时清缓存
 */
export function resetHighlightBuffer() {
  lineBuf = "";
}
