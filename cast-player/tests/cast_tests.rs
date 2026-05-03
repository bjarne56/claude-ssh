use cast_player_lib::cast_index::*;
use std::path::{Path, PathBuf};

/// 测试可选 cast 文件路径: 通过 TEST_CAST_FILE 环境变量传入
/// 不传则跳过 — 不在代码里硬编码任何用户路径
fn test_cast_path() -> Option<PathBuf> {
    std::env::var("TEST_CAST_FILE").ok().map(PathBuf::from).filter(|p| p.exists())
}

/// 用环境变量传入的真实录像文件测试 cast 索引构建
#[test]
fn test_build_index_from_real_cast() {
    let cast_path = match test_cast_path() {
        Some(p) => p,
        None => { eprintln!("跳过: 未设 TEST_CAST_FILE 环境变量"); return; }
    };
    let cast_path = cast_path.as_path();

    let index = CastIndex::build(cast_path).expect("构建索引失败");

    // 验证 header
    assert_eq!(index.header.version, 3, "cast 版本应为 3");
    assert!(index.total_duration > 0.0, "总时长应 > 0");

    // 验证事件数量
    assert!(index.events.len() > 10, "应该有不少于 10 个事件");

    // 验证 elapsed 递增
    for i in 1..index.events.len() {
        assert!(
            index.events[i].elapsed >= index.events[i - 1].elapsed,
            "elapsed 应单调递增: idx {} 处不满足",
            i
        );
    }

    // 验证 byte_offset 递增
    for i in 1..index.events.len() {
        assert!(
            index.events[i].byte_offset > index.events[i - 1].byte_offset,
            "byte_offset 应严格递增: idx {} 处不满足",
            i
        );
    }

    // 二分查找验证
    let mid = index.total_duration / 2.0;
    let found_idx = index.find_index_at(mid);
    assert!(found_idx < index.events.len(), "二分查找不应越界");
    assert!(
        index.events[found_idx].elapsed <= mid,
        "找到的 elapsed {} 应 <= target {}",
        index.events[found_idx].elapsed,
        mid
    );

    // 边界测试
    assert_eq!(index.find_index_at(0.0), 0);
    assert_eq!(index.find_index_at(index.total_duration + 100.0), index.events.len() - 1);
}

/// 测试 cast 文件流式分块读取
#[test]
fn test_read_chunk() {
    let Some(cast_path) = test_cast_path() else { return; };
    let cast_path = cast_path.as_path();
    let index = CastIndex::build(cast_path).unwrap();
    let start_offset = index.events[0].byte_offset;
    let chunk = CastIndex::read_chunk(cast_path, start_offset, None).unwrap();

    assert!(!chunk.lines.is_empty(), "chunk 不应为空");
    let first = &chunk.lines[0];
    let parsed: Vec<serde_json::Value> = serde_json::from_str(first.trim()).unwrap_or_default();
    assert!(parsed.len() >= 2, "每行至少 2 个元素 [delay, type]");
    assert!(parsed[0].is_number(), "第一个元素应为数字(delay)");
}

#[test]
fn test_load_meta() {
    let Some(cast_path) = test_cast_path() else { return; };
    let meta_path = cast_path.parent().unwrap().join("meta.json");
    if !meta_path.exists() { return; }

    let meta = load_meta(&meta_path).unwrap();
    assert!(!meta.session_id.is_empty(), "session_id 不能为空");
    assert!(!meta.host_resolved.is_empty(), "host_resolved 不能为空");
    assert!(!meta.started_at.is_empty(), "started_at 不能为空");
}

#[test]
fn test_load_commands() {
    let Some(cast_path) = test_cast_path() else { return; };
    let cmds_path = cast_path.parent().unwrap().join("commands.jsonl");
    if !cmds_path.exists() {
        return;
    }
    let cmds_path = cmds_path.as_path();

    let mut commands = load_commands(cmds_path).unwrap();
    sort_commands(&mut commands);

    // 验证排序
    for i in 1..commands.len() {
        assert!(
            commands[i].cast_offset >= commands[i - 1].cast_offset,
            "命令应按 cast_offset 排序"
        );
    }
}

/// 空 cast 文件(仅有 header)
#[test]
fn test_build_empty_cast() {
    use std::io::Write;

    let dir = std::env::temp_dir();
    let path = dir.join("empty_test.cast");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, r#"{{"version":3,"width":80,"height":24}}"#).unwrap();
    writeln!(f).unwrap();

    let index = CastIndex::build(&path).unwrap();
    assert_eq!(index.header.version, 3);
    assert!(index.events.is_empty());
    assert_eq!(index.total_duration, 0.0);

    let _ = std::fs::remove_file(&path);
}

/// 几个事件的简单 cast
#[test]
fn test_build_tiny_cast() {
    use std::io::Write;

    let dir = std::env::temp_dir();
    let path = dir.join("tiny_test.cast");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, r#"{{"version":3,"width":80,"height":24}}"#).unwrap();
    writeln!(f, r#"[0.1, "o", "hello"]"#).unwrap();
    writeln!(f, r#"[0.2, "o", " world"]"#).unwrap();
    writeln!(f, r#"[0.5, "i", "ls\r"]"#).unwrap();
    writeln!(f, r#"[0.0, "x", "0"]"#).unwrap();

    let index = CastIndex::build(&path).unwrap();
    assert_eq!(index.events.len(), 4, "应该有 4 个事件");
    assert!((index.total_duration - 0.8).abs() < 0.01, "总时长应为 0.8s ({})", index.total_duration);

    // 验证 elapsed 累加
    assert!((index.events[0].elapsed - 0.1).abs() < 0.01);
    assert!((index.events[1].elapsed - 0.3).abs() < 0.01);
    assert!((index.events[2].elapsed - 0.8).abs() < 0.01);
    assert!((index.events[3].elapsed - 0.8).abs() < 0.01);

    // 二分查找: target=0.15 应该定位到 idx 0 (events[0].elapsed=0.1 <= 0.15)
    assert_eq!(index.find_index_at(0.15), 0);
    // target=0.2: events[0].elapsed=0.1 <= 0.2 < events[1].elapsed=0.3 → idx 0
    assert_eq!(index.find_index_at(0.2), 0);
    // target=0.3: events[1].elapsed=0.3 精准命中
    assert_eq!(index.find_index_at(0.3), 1);
    // target=0.9 超过 0.8，应定位到最后一个
    assert_eq!(index.find_index_at(0.9), 3);

    let _ = std::fs::remove_file(&path);
}

/// verify real cast has balanced input/output events
#[test]
fn test_event_types() {
    let Some(cast_path) = test_cast_path() else { return; };
    let cast_path = cast_path.as_path();

    let index = CastIndex::build(cast_path).unwrap();

    let output_count = index.events.iter().filter(|e| e.event_type == EventType::Output).count();
    let input_count = index.events.iter().filter(|e| e.event_type == EventType::Input).count();
    let exit_count = index.events.iter().filter(|e| e.event_type == EventType::Exit).count();

    assert!(output_count > 0, "应该有输出事件");
    assert!(input_count > 0, "应该有输入事件");
    assert_eq!(exit_count, 1, "刚好 1 个退出事件");

    println!(
        "cast stats: {} events total, {} output, {} input, {} exit, {:.1}s",
        index.events.len(), output_count, input_count, exit_count, index.total_duration
    );
}