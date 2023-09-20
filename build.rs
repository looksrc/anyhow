#![allow(clippy::option_if_let_else)]

use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::str;

/// 如果开启了backtrace但是没开启std则直接报错,因为core不支持backtrace
#[cfg(all(feature = "backtrace", not(feature = "std")))]
compile_error! {
    "`backtrace` feature without `std` feature is not supported"
}

/// 如果当前的工具链指向一下代码不报错说明支持backstrace功能.
/// 目前为止,只有 nightly 支持错误处理中backstrace功能
/// This code exercises the surface area that we expect of the Error generic
/// member access API. If the current toolchain is able to compile it, then
///- anyhow is able to provide backtrace support.
const PROBE: &str = r#"
    #![feature(error_generic_member_access)]

    use std::backtrace::Backtrace;
    use std::error::{self, Error, Request};
    use std::fmt::{self, Debug, Display};

    struct MyError(Thing);
    struct Thing;

    impl Debug for MyError {
        fn fmt(&self, _formatter: &mut fmt::Formatter) -> fmt::Result {
            unimplemented!()
        }
    }

    impl Display for MyError {
        fn fmt(&self, _formatter: &mut fmt::Formatter) -> fmt::Result {
            unimplemented!()
        }
    }

    impl Error for MyError {
        fn provide<'a>(&'a self, request: &mut Request<'a>) {
            request.provide_ref(&self.0);
        }
    }

    const _: fn(&dyn Error) -> Option<&Backtrace> = |err| error::request_ref::<Backtrace>(err);
"#;

/// 主要功能
/// * 判断是否支持backtace,以确定是否传入--cfg=backtrace
/// * 判断是否支持std::ptr::addr_of,以确定是否传入--cfg=anyhow_no_ptr_addr_of
/// * 判断是否支持std::fmt::Arguments::as_str,以确定是否传入--cfg=anyhow_no_fmt_arguments_as_str
fn main() {
    // 探测当前工具链是否支持backtrace,如果支持则传入--cfg=backtrace
    if cfg!(feature = "std") {
        match compile_probe() {
            Some(status) if status.success() => println!("cargo:rustc-cfg=backtrace"),
            _ => {}
        }
    }

    // 当主版本为1时取次级版本号
    let rustc = match rustc_minor_version() {
        Some(rustc) => rustc,
        None => return,
    };

    // 没有 std::ptr::addr_of,传入--cfg=anyhow_no_ptr_addr_of
    if rustc < 51 {
        println!("cargo:rustc-cfg=anyhow_no_ptr_addr_of");
    }

    // 没有 std::fmt::Arguments::as_str,传入--cfg=anyhow_no_fmt_arguments_as_str
    if rustc < 52 {
        println!("cargo:rustc-cfg=anyhow_no_fmt_arguments_as_str");
    }
}

/// # 判断当前rust工具链是否支持 backtrace
/// 判断方法: 通过rustc执行PROBE代码, 如果支持 backtrace,则给编译器传入 --cfg=backtrace 选项开启 anyhow 的 backtrace 相关代码
fn compile_probe() -> Option<ExitStatus> {
    if env::var_os("RUSTC_STAGE").is_some() {
        // We are running inside rustc bootstrap. This is a highly non-standard
        // environment with issues such as:
        //
        //     https://github.com/rust-lang/cargo/issues/11138
        //     https://github.com/rust-lang/rust/issues/114839
        //
        // Let's just not use nightly features here.
        return None;
    }

    // 取 rustc 编译器可执行文件地址
    let rustc = env::var_os("RUSTC")?;
    // 取 build.rs 的默认输出目录
    let out_dir = env::var_os("OUT_DIR")?;
    // 取探测文件地址 OUTDIR/probe.rs,此文件用于探测当前工具链是否支持 backtrace,如果支持则传入 --cfg=backtrace
    let probefile = Path::new(&out_dir).join("probe.rs");
    // 向探测文件 probe.rs 写入探测代码 PROBE
    fs::write(&probefile, PROBE).ok()?;

    // 创建编译执行探测文件 probe.rs 的命令,
    // Make sure to pick up Cargo rustc configuration.
    let mut cmd = if let Some(wrapper) = env::var_os("RUSTC_WRAPPER") {
        let mut cmd = Command::new(wrapper);
        // The wrapper's first argument is supposed to be the path to rustc.
        cmd.arg(rustc);
        cmd
    } else {
        Command::new(rustc)
    };

    cmd.stderr(Stdio::null())
        .arg("--edition=2018")
        .arg("--crate-name=anyhow_build")
        .arg("--crate-type=lib")
        .arg("--emit=metadata")
        .arg("--out-dir")
        .arg(out_dir)
        .arg(probefile);

    if let Some(target) = env::var_os("TARGET") {
        cmd.arg("--target").arg(target);
    }

    // If Cargo wants to set RUSTFLAGS, use that.
    if let Ok(rustflags) = env::var("CARGO_ENCODED_RUSTFLAGS") {
        if !rustflags.is_empty() {
            for arg in rustflags.split('\x1f') {
                cmd.arg(arg);
            }
        }
    }

    cmd.status().ok()
}

/// # 求rustc的次级版本号
///
/// ## 方法:
/// - 通过command执行rusc --version命令并截取输出以求得rust版本号
/// - 如果rustc版本为rust1则返回rustc的取次级版本号,比如0.67
///
/// ## 格式:
/// - rustc 1.71.0 (8ede3aae2 2023-07-12)
/// - rustc 1.73.0-nightly (8771282d4 2023-07-23)
fn rustc_minor_version() -> Option<u32> {
    let rustc = env::var_os("RUSTC")?;
    let output = Command::new(rustc).arg("--version").output().ok()?;
    let version = str::from_utf8(&output.stdout).ok()?;
    let mut pieces = version.split('.');
    if pieces.next() != Some("rustc 1") {
        return None;
    }
    pieces.next()?.parse().ok()
}
