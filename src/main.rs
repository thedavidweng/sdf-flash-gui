// sdf-flash-gui — Cross-platform SDFtool GUI for optical drive firmware.

use sdf_flash_gui::command;
use sdf_flash_gui::drive;
use sdf_flash_gui::manifest;
use sdf_flash_gui::orchestration;
use sdf_flash_gui::sdf;

fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();

    if args.len() <= 1 {
        if let Err(e) = sdf_flash_gui::gui::run() {
            eprintln!("GUI error: {e}");
            std::process::exit(1);
        }
        return;
    }

    match args[1].as_str() {
        "list" => cmd_list(),
        "info" => {
            if args.len() < 3 {
                eprintln!("Usage: sdf-flash-gui info <device>");
                std::process::exit(1);
            }
            cmd_info(&args[2]);
        }
        "dump" => {
            if args.len() < 3 {
                eprintln!("Usage: sdf-flash-gui dump <device> -o <output_dir>");
                std::process::exit(1);
            }
            let output_dir = args
                .iter()
                .position(|a| a == "-o")
                .and_then(|i| args.get(i + 1))
                .map(String::as_str)
                .unwrap_or(".");
            cmd_dump(&args[2], output_dir);
        }
        "flash" => {
            if args.len() < 3
                || args.contains(&"--help".to_string())
                || args.contains(&"-h".to_string())
            {
                print_flash_help();
                if args.len() < 3 {
                    std::process::exit(1);
                }
                return;
            }
            cmd_flash(
                &args[2],
                FlashArgs {
                    firmware: args
                        .iter()
                        .position(|a| a == "-i")
                        .and_then(|i| args.get(i + 1)),
                    manifest_path: args
                        .iter()
                        .position(|a| a == "--manifest")
                        .and_then(|i| args.get(i + 1)),
                    image_id: args
                        .iter()
                        .position(|a| a == "--image-id")
                        .and_then(|i| args.get(i + 1)),
                    sdf_path: args
                        .iter()
                        .position(|a| a == "--sdf")
                        .and_then(|i| args.get(i + 1)),
                    encrypted: args.contains(&"--encrypted".to_string()),
                    include_boot_loader: args.contains(&"--include-boot-loader".to_string()),
                    recover: args.contains(&"--recover".to_string()),
                    wrong_firmware: args
                        .iter()
                        .position(|a| a == "--wrong-firmware")
                        .and_then(|i| args.get(i + 1)),
                    recovery_token: args
                        .iter()
                        .position(|a| a == "--recovery-token")
                        .and_then(|i| args.get(i + 1)),
                    confirm: args.contains(&"--confirm".to_string()),
                },
            );
        }
        "sdf-info" => {
            let file = args
                .iter()
                .position(|a| a == "--file" || a == "-f")
                .and_then(|i| args.get(i + 1));
            match file {
                Some(f) => cmd_sdf_info(f),
                None => {
                    eprintln!("Usage: sdf-flash-gui sdf-info --file <sdf.bin>");
                    std::process::exit(1);
                }
            }
        }
        "--help" | "-h" | "help" => print_help(),
        other => {
            eprintln!("Unknown command: {other}");
            print_help();
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        "{} — Cross-platform SDFtool GUI",
        sdf_flash_gui::branding::DISPLAY_NAME
    );
    println!();
    println!("USAGE:");
    println!("  sdf-flash-gui                        Launch GUI");
    println!("  sdf-flash-gui list                   List optical drives");
    println!("  sdf-flash-gui info <device>          Show drive info");
    println!("  sdf-flash-gui dump <device> -o DIR   Dump firmware");
    println!("  sdf-flash-gui flash <device> -i FW   Flash firmware (dry-run)");
    println!("  sdf-flash-gui sdf-info --file FILE   Parse SDF0 container");
    println!();
    println!("OPTIONS:");
    println!("  -i <file>          Firmware image path");
    println!("  -o <dir>           Output directory");
    println!("  --manifest <file>  Firmware manifest JSON");
    println!("  --confirm          Confirm flash operation");
    println!("  --file <file>      SDF0 container path");
    println!();
    println!("Run 'sdf-flash-gui flash --help' for flash-specific options.");
}

fn print_flash_help() {
    println!("USAGE: sdf-flash-gui flash <device> -i <firmware> [options]");
    println!();
    println!("OPTIONS:");
    println!("  -i <file>                  Firmware image path (required)");
    println!("  --manifest <file>          Firmware manifest JSON");
    println!("  --image-id <id>            Image ID for multi-image manifests");
    println!("  --sdf <file>               Path to sdf.bin (auto-detected if omitted)");
    println!("  --encrypted                Use encrypted rawflash mode");
    println!("  --include-boot-loader      Use full boot-loader rawflash mode");
    println!("  --recover                  Recovery flash mode");
    println!("  --wrong-firmware <file>    Extract recovery boot token from file");
    println!("  --recovery-token <token>   16-byte recovery boot token");
    println!("  --confirm                  Confirm and execute (otherwise dry-run)");
}

fn find_backend() -> (command::Backend, String) {
    drive::find_backend().unwrap_or_else(|| {
        eprintln!("ERROR: sdftool or makemkvcon not found. Install MakeMKV or sdftool.");
        std::process::exit(1);
    })
}

fn find_sdf_bin() -> String {
    drive::find_sdf_bin()
}

fn cmd_list() {
    let drives = drive::enumerate_drives();
    if drives.is_empty() {
        println!("No optical drives detected.");
        let (backend, path) = find_backend();
        match orchestration::run_list_backend(backend, &path) {
            Ok(out) => print!("{out}"),
            Err(e) => eprintln!("ERROR: {e}"),
        }
    } else {
        println!("Optical Drives:");
        for d in &drives {
            if d.vendor.is_empty() {
                println!("  {}", d.device);
            } else {
                println!("  {} {} {} ({})", d.device, d.vendor, d.product, d.revision);
            }
        }
    }
}

fn cmd_info(device: &str) {
    let (backend, path) = find_backend();
    match orchestration::probe_drive(backend, &path, device) {
        Ok(probe) => {
            print!("{}", probe.output);
            if !probe.output.is_empty() {
                println!();
            }
            println!("Platform: MT1959={}", probe.safety.mt1959);
            println!("Encrypted firmware: {}", probe.safety.encrypted_firmware);
            if let Some(prefix) = probe.safety.firmware_date_prefix {
                println!("Firmware date prefix: {prefix}");
            }
            if let Some(mode) = probe.safety.mtk_mode {
                println!("MTK mode: {mode}");
            }
            if !probe.identity.vendor.is_empty() || !probe.identity.model.is_empty() {
                println!(
                    "Identity: {} {} {}",
                    probe.identity.vendor, probe.identity.model, probe.identity.revision
                );
            }
        }
        Err(e) => eprintln!("ERROR: {e}"),
    }
}

fn validate_output_dir(path: &str) -> Result<(), String> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(format!("output directory does not exist: {path}"));
    }
    if !p.is_dir() {
        return Err(format!("output path is not a directory: {path}"));
    }
    let probe = p.join(".sdf-flash-gui-write-test");
    std::fs::write(&probe, b"").map_err(|e| format!("output directory is not writable: {e}"))?;
    let _ = std::fs::remove_file(probe);
    Ok(())
}

fn cmd_dump(device: &str, output_dir: &str) {
    if let Err(e) = validate_output_dir(output_dir) {
        eprintln!("ERROR: {e}");
        std::process::exit(1);
    }
    let (backend, path) = find_backend();
    let sdf_path = find_sdf_bin();
    match orchestration::run_dump(backend, &path, &sdf_path, device, output_dir) {
        Ok(()) => println!("Firmware dumped to {output_dir}"),
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}

struct FlashArgs<'a> {
    firmware: Option<&'a String>,
    manifest_path: Option<&'a String>,
    image_id: Option<&'a String>,
    sdf_path: Option<&'a String>,
    encrypted: bool,
    include_boot_loader: bool,
    recover: bool,
    wrong_firmware: Option<&'a String>,
    recovery_token: Option<&'a String>,
    confirm: bool,
}

fn cmd_flash(device: &str, args: FlashArgs<'_>) {
    let firmware = match args.firmware {
        Some(f) => f.as_str(),
        None => {
            eprintln!("ERROR: -i <firmware> is required");
            std::process::exit(1);
        }
    };

    let firmware_data = match std::fs::read(firmware) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ERROR: cannot read firmware: {e}");
            std::process::exit(1);
        }
    };

    let manifest = args.manifest_path.map(|mp| {
        let data = match std::fs::read(mp) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("ERROR: cannot read manifest: {e}");
                std::process::exit(1);
            }
        };
        match manifest::parse_manifest(&data) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("ERROR: invalid manifest: {e}");
                std::process::exit(1);
            }
        }
    });

    let (backend, path) = find_backend();
    let sdf_path = args.sdf_path.map(String::as_str).unwrap_or_else(|| {
        let auto = find_sdf_bin();
        if auto.is_empty() {
            ""
        } else {
            // Leak the string to get a 'static str — this is a CLI short-lived process
            Box::leak(auto.into_boxed_str())
        }
    });
    let session = match orchestration::FlashSession::prepare(orchestration::FlashSessionRequest {
        backend,
        tool_path: &path,
        sdf_path,
        device,
        firmware_path: firmware,
        firmware_data: &firmware_data,
        manifest: manifest.as_ref(),
        manifest_path: args.manifest_path.map(String::as_str),
        image_id: args.image_id.map(String::as_str),
        encrypted: args.encrypted,
        include_boot_loader: args.include_boot_loader,
        recover: args.recover,
        wrong_firmware: args.wrong_firmware.map(String::as_str),
        recovery_token: args.recovery_token.map(String::as_str),
        confirm: if args.confirm {
            orchestration::FlashConfirm::Flag
        } else {
            orchestration::FlashConfirm::None
        },
        lang: sdf_flash_gui::i18n::Language::English,
    }) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    for w in &session.no_manifest_warnings {
        eprintln!("WARNING: {w}");
    }

    if let Some(report) = &session.report {
        for w in &report.warnings {
            eprintln!("WARNING: {w}");
        }
        println!("{}", report.summary);
        if !session.would_execute {
            if !args.confirm {
                println!("Add --confirm to proceed.");
            }
            std::process::exit(1);
        }
    } else if !args.confirm {
        eprintln!("Add --confirm to proceed without manifest validation.");
        std::process::exit(1);
    }

    if !args.confirm {
        println!("Dry-run complete. Add --confirm to proceed.");
        return;
    }

    match session.execute() {
        Ok(()) => println!("Flash completed successfully."),
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_sdf_info(file: &str) {
    let data = match std::fs::read(file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ERROR: cannot read {file}: {e}");
            std::process::exit(1);
        }
    };

    let mut cursor = std::io::Cursor::new(&data);
    match sdf::parse_sdf0(&mut cursor) {
        Ok(container) => print!("{}", sdf::format_container_cli(&container, file)),
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}
