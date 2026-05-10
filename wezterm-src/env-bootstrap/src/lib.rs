pub mod ringlog;
pub use ringlog::setup_logger;
use std::path::{Path, PathBuf};

pub fn set_wezterm_executable() {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            std::env::set_var("WEZTERM_EXECUTABLE_DIR", dir);
        }
        std::env::set_var("WEZTERM_EXECUTABLE", exe);
    }
}

pub fn fixup_snap() {
    if std::env::var_os("SNAP").is_some() {
        // snapd sets a bunch of environment variables as a part of setup
        // These are not useful to be passed through to things spawned by us.

        // SNAP is the base path of the files in the snap
        std::env::remove_var("SNAP");

        // snapd also sets a bunch of SNAP_* environment variables
        // This list may change over time, so for simplicity, assume
        // anything in the SNAP_* namespace is set by snapd, and unset it.
        std::env::vars_os()
            .into_iter()
            .filter(|(key, _)| {
                key.to_str()
                    .filter(|key| key.starts_with("SNAP_"))
                    .is_some()
            })
            .map(|(key, _)| key)
            .for_each(|key| std::env::remove_var(key));

        // snapd has *also* set LD_LIBRARY_PATH, and things we spawn
        // *absolutely do not* want this propagated
        std::env::remove_var("LD_LIBRARY_PATH");
    }
}

pub fn fixup_appimage() {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        let appimage = std::path::PathBuf::from(appimage);

        // We were started via an AppImage, presumably ourselves.
        // AppImage exports ARGV0 into the environment and that causes
        // everything that was indirectly spawned by us to appear to
        // be the AppImage.  eg: if you `vim foo` it shows as
        // `WezTerm.AppImage foo`, which is super confusing for everyone!
        // Let's just unset that from the environment!
        std::env::remove_var("ARGV0");

        // Since our AppImage includes multiple utilities, we want to
        // be able to use them, so add that location to the PATH!
        // WEZTERM_EXECUTABLE_DIR is set by `set_wezterm_executable`
        // which is called before `fixup_appimage`
        if let Some(dir) = std::env::var_os("WEZTERM_EXECUTABLE_DIR") {
            if let Some(path) = std::env::var_os("PATH") {
                let mut paths = std::env::split_paths(&path).collect::<Vec<_>>();
                paths.insert(0, PathBuf::from(dir));
                let new_path = std::env::join_paths(paths).expect("unable to update PATH");
                std::env::set_var("PATH", &new_path);
            }
        }

        // This AppImage feature allows redirecting HOME and XDG_CONFIG_HOME
        // to live alongside the executable for portable use:
        // https://github.com/AppImage/AppImageKit/issues/368
        // When we spawn children, we don't want them to inherit this,
        // but we do want to respect them for config loading.
        // Let's force resolution and cleanup our environment now.

        /// Given "/some/path.AppImage" produce "/some/path.AppImageSUFFIX".
        /// We only support this for "path.AppImage" that can be converted
        /// to UTF-8.  Otherwise, we return "/some/path.AppImage" unmodified.
        fn append_extra_file_name_suffix(p: &Path, suffix: &str) -> PathBuf {
            if let Some(name) = p.file_name().and_then(|o| o.to_str()) {
                p.with_file_name(format!("{}{}", name, suffix))
            } else {
                p.to_path_buf()
            }
        }

        /// Our config stuff exports these env vars to help portable apps locate
        /// the correct environment when it is launched via wezterm.
        /// However, if we are using the system wezterm to spawn a portable
        /// AppImage then we want these to not take effect.
        fn clean_wezterm_config_env() {
            std::env::remove_var("WEZTERM_CONFIG_FILE");
            std::env::remove_var("WEZTERM_CONFIG_DIR");
        }

        if config::HOME_DIR.starts_with(append_extra_file_name_suffix(&appimage, ".home")) {
            // Fixup HOME to point to the user's actual home dir
            std::env::remove_var("HOME");
            std::env::set_var(
                "HOME",
                dirs_next::home_dir().expect("can't resolve HOME dir"),
            );
            clean_wezterm_config_env();
        }

        if std::env::var("XDG_CONFIG_HOME")
            .map(|d| {
                PathBuf::from(d).starts_with(append_extra_file_name_suffix(&appimage, ".config"))
            })
            .unwrap_or_default()
        {
            std::env::remove_var("XDG_CONFIG_HOME");
            clean_wezterm_config_env();
        }
    }
}

/// If LANG isn't set in the environment, make an attempt at setting
/// it to a UTF-8 version of the current locale known to NSLocale.
#[cfg(target_os = "macos")]
pub fn set_lang_from_locale() {
    use objc2_foundation::{NSLocale, NSString};

    fn lang_is_set() -> bool {
        match std::env::var_os("LANG") {
            None => false,
            Some(lang) => !lang.is_empty(),
        }
    }

    if !lang_is_set() {
        unsafe fn nsstring_to_str<'a>(ns: &NSString) -> &'a str {
            let data = ns.UTF8String() as *const u8;
            let len = ns.len();
            let bytes = std::slice::from_raw_parts(data, len);
            std::str::from_utf8_unchecked(bytes)
        }

        unsafe {
            let locale = NSLocale::autoupdatingCurrentLocale();
            let lang_code_obj = locale.languageCode();
            let lang_code = nsstring_to_str(&lang_code_obj);

            #[allow(deprecated)]
            let candidate = if let Some(country_code_obj) = locale.countryCode() {
                let country_code = nsstring_to_str(&country_code_obj);
                format!("{}_{}.UTF-8", lang_code, country_code)
            } else {
                format!("{}.UTF-8", lang_code)
            };

            let candidate_cstr =
                std::ffi::CString::new(candidate.as_bytes()).expect("make cstr from str");

            // If this looks like a working locale then export it to
            // the environment so that our child processes inherit it.
            let old = libc::setlocale(libc::LC_CTYPE, std::ptr::null());
            if !libc::setlocale(libc::LC_CTYPE, candidate_cstr.as_ptr()).is_null() {
                std::env::set_var("LANG", &candidate);
            } else {
                log::debug!("setlocale({}) failed, fall back to en_US.UTF-8", candidate);
                std::env::set_var("LANG", "en_US.UTF-8");
            }
            libc::setlocale(libc::LC_CTYPE, old);
        }
    }
}

/// 判定 locale 字符串是否属于"远端 unsafe"集合, 需要升级到 en_US.UTF-8.
///
/// 命中三类:
/// - 裸字符集 (UTF-8 / utf-8 / UTF8 / utf8): macOS BSD libc 接受, Linux
///   glibc 不认 (setlocale 失败).
/// - C.UTF-8 / C.utf-8 / C.utf8: glibc 2.13+ 支持, 但 RedHat 系长期没
///   backport, CentOS 7 (glibc 2.17) 的 locale -a 里没有, setlocale 失败.
/// - 空字符串: 等同未设, 远端 setlocale 拿到空也走 fallback.
///
/// 完整 locale 形式 (xx_YY.UTF-8 / zh_CN.UTF-8 等) 不命中, 优先保留用户
/// 显式选择.
fn is_remote_unsafe_locale(value: &str) -> bool {
    matches!(
        value,
        "" | "UTF-8" | "utf-8" | "UTF8" | "utf8" | "C.UTF-8" | "C.utf-8" | "C.utf8"
    )
}

/// 兜底注入 LC_CTYPE=en_US.UTF-8.
///
/// macOS launchd / 一般 GUI 启动链不传 LC_CTYPE, ssh 子进程透传到远端时
/// glibc 拿不到 UTF-8 字符集 → vim/less 默认 encoding=latin1, UTF-8 文件
/// 按字节流显示成 ~大写字母 乱码 (典型 wezterm-ssh 远端中文乱码症状).
///
/// 选 en_US.UTF-8 而不是 C.UTF-8 或裸 UTF-8 的原因:
/// - 裸 UTF-8: macOS BSD libc 接受, Linux glibc 不认 (setlocale 失败).
/// - C.UTF-8: glibc 2.13+ 支持但 RedHat 系长期没 backport, CentOS 7
///   (glibc 2.17) locale -a 里没有, setlocale 失败 → vim 仍 latin1.
/// - en_US.UTF-8: glibc 全系列 (含 CentOS 6/7) 都有, macOS BSD libc 也认,
///   musl 支持. 跨发行版兼容性最好.
///
/// LC_CTYPE 只控制字符编码 + ctype 函数 (isupper/tolower 等), 不影响
/// LC_MESSAGES (应用语言). 远端用户 LANG / LC_MESSAGES 由远端 shell rc /
/// login env 决定, 不会被此值强制成英文.
///
/// 触发条件:
/// 1. LC_CTYPE 完全未设 → 设为 en_US.UTF-8
/// 2. LC_CTYPE 是裸字符集形式 (UTF-8 / utf-8 / UTF8 / utf8) → 升级到
///    en_US.UTF-8 (Apple Terminal.app / iTerm2 / WezTerm 上游默认注入裸
///    UTF-8, macOS BSD libc 接受, 但 SSH 透传到 Linux 远端无效.)
/// 3. LC_CTYPE = C.UTF-8 → 升级到 en_US.UTF-8 (兼容 CentOS 7 等无 C.UTF-8
///    的 glibc; 在支持 C.UTF-8 的系统上 en_US.UTF-8 同样是 valid utf8 locale,
///    无副作用.)
///
/// 其他完整 locale 形式 (xx_YY.UTF-8) 优先, 不覆盖.
fn set_lc_ctype_default() {
    let needs_upgrade = match std::env::var_os("LC_CTYPE") {
        None => true,
        Some(v) => is_remote_unsafe_locale(v.to_string_lossy().as_ref()),
    };
    if needs_upgrade {
        std::env::set_var("LC_CTYPE", "en_US.UTF-8");
    }
}

/// 兜底注入 LANG=en_US.UTF-8, 与 set_lc_ctype_default 对称处理.
///
/// 单独修 LC_CTYPE 不够: glibc 的 setlocale(LC_ALL, "") 是原子调用 ——
/// 任一 LC_X 来源 (LC_ALL > LC_X > LANG) 是 invalid locale 整个调用失败,
/// 所有 LC_* 一起 fallback 到 C, 即使 LC_CTYPE 自身合法也救不回来.
///
/// 大富 macOS shell env 实测 LANG=C.UTF-8 (zsh 继承自 launchd), 透传到
/// CentOS 7 远端 → setlocale("") 失败 → vim encoding=latin1, 哪怕我们
/// 已经设了 LC_CTYPE=en_US.UTF-8 也照样乱码.
///
/// 触发条件同 set_lc_ctype_default (复用 is_remote_unsafe_locale):
/// 1. LANG 未设
/// 2. LANG = "" / 裸 UTF-8 / C.UTF-8 及变体
/// → 升级到 en_US.UTF-8
///
/// 其他完整 locale (zh_CN.UTF-8 / ja_JP.UTF-8 等用户显式选择) 不动, 即便
/// 远端可能没装这些 locale 也属于用户责任 —— 只兜远端肯定不工作的几种
/// invalid 形式.
///
/// 副作用: 远端 LC_MESSAGES 默认从 LANG 取, 升级到 en_US.UTF-8 会让远端
/// 程序 (systemctl, ls -l, error msg) 输出英文. 对运维场景可接受 ——
/// 中文 vim 显示远比中文 menu 重要; 远端用户若需要中文 menu 可在
/// ~/.bashrc 显式 export LANG=zh_CN.UTF-8 覆盖.
fn set_lang_default() {
    let needs_upgrade = match std::env::var_os("LANG") {
        None => true,
        Some(v) => is_remote_unsafe_locale(v.to_string_lossy().as_ref()),
    };
    if needs_upgrade {
        std::env::set_var("LANG", "en_US.UTF-8");
    }
}

fn register_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let payload = payload.downcast_ref::<&str>().unwrap_or(&"!?");
        let bt = backtrace::Backtrace::new();
        if let Some(loc) = info.location() {
            log::error!(
                "panic at {}:{}:{} - {}\n{:?}",
                loc.file(),
                loc.line(),
                loc.column(),
                payload,
                bt
            );
        } else {
            log::error!("panic - {}\n{:?}", payload, bt);
        }
        default_hook(info);
    }));
}

fn register_lua_modules() {
    for func in [
        battery::register,
        color_funcs::register,
        termwiz_funcs::register,
        logging::register,
        mux_lua::register,
        procinfo_funcs::register,
        filesystem::register,
        serde_funcs::register,
        plugin::register,
        ssh_funcs::register,
        spawn_funcs::register,
        share_data::register,
        time_funcs::register,
        url_funcs::register,
    ] {
        config::lua::add_context_setup_func(func);
    }
}

pub fn bootstrap() {
    config::assign_version_info(
        wezterm_version::wezterm_version(),
        wezterm_version::wezterm_target_triple(),
    );
    setup_logger();
    register_panic_hook();

    set_wezterm_executable();

    #[cfg(target_os = "macos")]
    set_lang_from_locale();

    set_lc_ctype_default();
    set_lang_default();

    fixup_appimage();
    fixup_snap();

    register_lua_modules();

    // Remove this env var to avoid weirdness with some vim configurations.
    // wezterm never sets WINDOWID and we don't want to inherit it from a
    // parent process.
    std::env::remove_var("WINDOWID");
    // Avoid vte shell integration kicking in if someone started
    // wezterm or the mux server from inside gnome terminal.
    // <https://github.com/wezterm/wezterm/issues/2237>
    std::env::remove_var("VTE_VERSION");

    // Sice folks don't like to reboot or sign out if they `chsh`,
    // SHELL may be stale. Rather than using a stale value, unset
    // it so that pty::CommandBuilder::get_shell will resolve the
    // shell from the password database instead.
    std::env::remove_var("SHELL");
}
