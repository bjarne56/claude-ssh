use cast_player_lib::cast_index::*;
use std::path::Path;

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