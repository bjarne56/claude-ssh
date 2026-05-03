use cast_player_lib::cast_index::*;
use std::path::Path;

#[test]
fn test_extract_human_ifconfig_command() {
    let dir = Path::new("/Users/bjarne/Code/ssh-op/vedio/test2_/10.32.49.7-20260503-065946-f65eae");
    let cast = dir.join("stream.cast");
    let cmds_path = dir.join("commands.jsonl");
    if !cast.exists() {
        eprintln!("跳过: 文件不存在");
        return;
    }

    let ai_cmds = load_commands(&cmds_path).unwrap();
    let events = CastIndex::read_all_events(&cast).unwrap();

    let merged = merge_commands_with_inputs(ai_cmds.clone(), &events);

    println!("ai 命令数: {}", ai_cmds.len());
    println!("合并后总数: {}", merged.len());
    for cmd in &merged {
        println!("  [{}] @{:.1}s (input@{:.1}s) {}", cmd.actor, cmd.cast_offset, cmd.input_start_offset, cmd.cmd);
    }

    // 应该比 ai 命令多 (因为加了 human 命令)
    assert!(merged.len() > ai_cmds.len(), "应该有 human 命令被加入");

    // 应该含 ifconfig
    let has_ifconfig = merged.iter().any(|c| c.cmd.contains("ifconfig"));
    assert!(has_ifconfig, "应该提取到 ifconfig 命令");

    // ifconfig 应该是 human, 且 input_start_offset > 0
    let ifc = merged.iter().find(|c| c.cmd.contains("ifconfig")).unwrap();
    assert_eq!(ifc.actor, "human");
    assert!(ifc.input_start_offset > 0.0);
    assert!(ifc.input_start_offset < ifc.cast_offset);
}


#[test]
fn test_real_session_sudo_i_classified_as_ai() {
    // sudo -i / export PS1 是 ssh-ops 一次发整行 → max_chunk_size > 3 → 应分类为 ai
    let dir = Path::new("/Users/bjarne/Code/ssh-op/vedio/test2_/10.32.49.7-20260503-065946-f65eae");
    let cast = dir.join("stream.cast");
    let cmds_path = dir.join("commands.jsonl");
    if !cast.exists() {
        return;
    }
    let ai_cmds = load_commands(&cmds_path).unwrap();
    let events = CastIndex::read_all_events(&cast).unwrap();
    let merged = merge_commands_with_inputs(ai_cmds, &events);

    let sudo = merged.iter().find(|c| c.cmd == "sudo -i").expect("sudo -i 应在列表");
    assert_eq!(sudo.actor, "ai", "sudo -i 整块发送应归 ai");

    let pset = merged.iter().find(|c| c.cmd.contains("export PS1=")).expect("PS1 命令应在");
    assert_eq!(pset.actor, "ai", "export PS1 整块发送应归 ai");

    let ifc = merged.iter().find(|c| c.cmd == "ifconfig").expect("ifconfig 应在");
    assert_eq!(ifc.actor, "human", "ifconfig 逐字符应归 human");
}

#[test]
fn test_input_group_injection_detection() {
    let events_paste: Vec<(f64, String)> = vec![
        (1.0, r#"[1.0,"i","ls -la /tmp\r"]"#.into()),
    ];
    let groups = extract_input_groups(&events_paste);
    assert_eq!(groups.len(), 1);
    assert!(groups[0].is_injected(), "单事件含整命令应判 injected");

    let events_typed: Vec<(f64, String)> = vec![
        (1.0, r#"[1.0,"i","l"]"#.into()),
        (0.1, r#"[0.1,"i","s"]"#.into()),
        (0.1, r#"[0.1,"i","\r"]"#.into()),
    ];
    let groups = extract_input_groups(&events_typed);
    assert_eq!(groups.len(), 1);
    assert!(!groups[0].is_injected(), "单字符事件应判非 injected");
    assert_eq!(groups[0].chunk_count, 3);
}

#[test]
fn test_extract_input_groups_handles_backspace() {
    let events: Vec<(f64, String)> = vec![
        (0.0, r#"[0.0,"i","i"]"#.into()),
        (0.1, r#"[0.1,"i","c"]"#.into()),
        (0.1, r#"[0.1,"i","p"]"#.into()),
        (0.2, r#"[0.2,"i",""]"#.into()),
        (0.1, r#"[0.1,"i",""]"#.into()),
        (0.1, r#"[0.1,"i","p"]"#.into()),
        (0.1, r#"[0.1,"i"," "]"#.into()),
        (0.1, r#"[0.1,"i","a"]"#.into()),
        (0.1, r#"[0.1,"i","\r"]"#.into()),
    ];
    let groups = extract_input_groups(&events);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].content, "ip a");
    assert!(!groups[0].is_injected(), "逐字符 + 退格应是 human");
}

#[test]
fn test_load_user_real_session() {
    let dir = Path::new("/Users/bjarne/Code/ssh-op/vedio/test2_/10.32.49.7-20260503-074246-f078fe");
    let cast = dir.join("stream.cast");
    let meta_path = dir.join("meta.json");
    let cmds_path = dir.join("commands.jsonl");

    assert!(cast.exists(), "cast 文件不存在");
    assert!(meta_path.exists(), "meta.json 不存在");
    assert!(cmds_path.exists(), "commands.jsonl 不存在");

    // 1. 加载 meta
    let meta = load_meta(&meta_path).expect("加载 meta 失败");
    println!("\n=== META ===");
    println!("session_id: {}", meta.session_id);
    println!("host: {}", meta.host_resolved);
    println!("command_count: {}", meta.command_count);
    assert_eq!(meta.command_count, 9, "应该有 9 条命令");

    // 2. 加载命令
    let mut cmds = load_commands(&cmds_path).expect("加载命令失败");
    sort_commands(&mut cmds);
    println!("\n=== COMMANDS ({}) ===", cmds.len());
    for c in &cmds {
        println!("  offset={:.1}s exit={} cmd={}", c.cast_offset, c.exit, &c.cmd[..c.cmd.len().min(60)]);
    }
    assert_eq!(cmds.len(), 9);

    // 3. 构建 cast 索引 (这是性能关键路径)
    let start = std::time::Instant::now();
    let index = CastIndex::build(&cast).expect("构建索引失败");
    println!("\n=== CAST INDEX ===");
    println!("events: {}", index.events.len());
    println!("total_duration: {:.1}s", index.total_duration);
    println!("build time: {:?}", start.elapsed());

    assert!(index.events.len() > 100, "应该有不少事件");
    assert!(index.total_duration > 1000.0, "总时长应大于 1000s");

    // 4. 验证 seek 到每条命令的偏移都能找到正确事件
    for cmd in &cmds {
        let idx = index.find_index_at(cmd.cast_offset);
        assert!(idx < index.events.len(), "命令 offset {} 二分查找越界", cmd.cast_offset);
        let found_elapsed = index.events[idx].elapsed;
        println!(
            "seek({:.1}) → idx {} (elapsed={:.3})",
            cmd.cast_offset, idx, found_elapsed
        );
    }

    // 5. 加载所有 cast 事件 (前端会做这个,需要确保不会爆栈/超慢)
    let start = std::time::Instant::now();
    let events = CastIndex::read_all_events(&cast).expect("读全部事件失败");
    println!(
        "\n=== read_all_events: {} events in {:?} ===",
        events.len(),
        start.elapsed()
    );
    assert_eq!(events.len(), index.events.len(), "事件数应一致");

    // 6. 验证每个事件都是合法 JSON
    for (i, (_delay, line)) in events.iter().enumerate() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(line.trim());
        assert!(parsed.is_ok(), "事件 {} 不是合法 JSON: {}", i, line);
    }
}