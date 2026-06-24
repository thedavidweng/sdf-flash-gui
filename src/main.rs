// sdf-flash-gui — Cross-platform SDFtool GUI for optical drive firmware.

mod command;
mod drive;
mod flash;
mod gui;
mod i18n;
mod manifest;
mod sdf;

use std::process;

fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();

    if args.len() <= 1 {
        // No arguments — launch GUI
        if let Err(e) = gui::run() {
            eprintln!("GUI error: {e}");
            process::exit(1);
        }
        return;
    }

    // CLI mode
    match args[1].as_str() {
        "list" => cmd_list(),
        "info" => {
            if args.len() < 3 {
                eprintln!("Usage: sdf-flash-gui info <device>");
                process::exit(1);
            }
            cmd_info(&args[2]);
        }
        "dump" => {
            if args.len() < 3 {
                eprintln!("Usage: sdf-flash-gui dump <device> -o <output_dir>");
                process::exit(1);
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
                    process::exit(1);
                }
                return;
            }
            let firmware = args
                .iter()
                .position(|a| a == "-i")
                .and_then(|i| args.get(i + 1));
            let manifest_path = args
                .iter()
                .position(|a| a == "--manifest")
                .and_then(|i| args.get(i + 1));
            let image_id = args
                .iter()
                .position(|a| a == "--image-id")
                .and_then(|i| args.get(i + 1));
            let encrypted = args.contains(&"--encrypted".to_string());
            let include_boot_loader = args.contains(&"--include-boot-loader".to_string());
            let recover = args.contains(&"--recover".to_string());
            let wrong_firmware = args
                .iter()
                .position(|a| a == "--wrong-firmware")
                .and_then(|i| args.get(i + 1));
            let recovery_token = args
                .iter()
                .position(|a| a == "--recovery-token")
                .and_then(|i| args.get(i + 1));
            let confirm = args.contains(&"--confirm".to_string());
            cmd_flash(
                &args[2],
                FlashArgs {
                    firmware,
                    manifest_path,
                    image_id,
                    encrypted,
                    include_boot_loader,
                    recover,
                    wrong_firmware,
                    recovery_token,
                    confirm,
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
                    process::exit(1);
                }
            }
        }
        "--help" | "-h" | "help" => {
            print_help();
        }
        other => {
            eprintln!("Unknown command: {other}");
            print_help();
            process::exit(1);
        }
    }
}

fn print_help() {
    println!("sdf-flash-gui — Cross-platform SDFtool GUI");
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
        process::exit(1);
    })
}

fn cmd_list() {
    let drives = drive::enumerate_drives();
    if drives.is_empty() {
        println!("No optical drives detected.");
        // Try backend
        let (backend, path) = find_backend();
        let cmd = command::plan_drive_list(backend, &path);
        match drive::run_command(&cmd.program, &cmd.args) {
            Ok(out) => print!("{}", out.combined()),
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
    let cmd = command::plan_drive_info(backend, &path, device);
    match drive::run_command(&cmd.program, &cmd.args) {
        Ok(out) => {
            print!("{}", out.combined());
            let safety = command::classify_drive_safety(device, &out.combined());
            println!();
            println!("Platform: MT1959={}", safety.mt1959);
            println!("Encrypted firmware: {}", safety.encrypted_firmware);
            if let Some(prefix) = safety.firmware_date_prefix {
                println!("Firmware date prefix: {prefix}");
            }
            if let Some(mode) = safety.mtk_mode {
                println!("MTK mode: {mode}");
            }
        }
        Err(e) => eprintln!("ERROR: {e}"),
    }
}

fn cmd_dump(device: &str, output_dir: &str) {
    let (backend, path) = find_backend();
    let cmd = command::Command {
        program: path,
        args: if backend == command::Backend::MakeMkvCon {
            vec![
                "f".into(),
                "-d".into(),
                device.into(),
                "dump".into(),
                "auto".into(),
                "-o".into(),
                output_dir.into(),
            ]
        } else {
            vec![
                "-d".into(),
                device.into(),
                "dump".into(),
                "auto".into(),
                "-o".into(),
                output_dir.into(),
            ]
        },
    };
    match drive::run_command(&cmd.program, &cmd.args) {
        Ok(out) => {
            if out.success() {
                println!("Firmware dumped to {output_dir}");
            } else {
                eprintln!("ERROR: {}", out.combined());
                process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            process::exit(1);
        }
    }
}

struct FlashArgs<'a> {
    firmware: Option<&'a String>,
    manifest_path: Option<&'a String>,
    image_id: Option<&'a String>,
    encrypted: bool,
    include_boot_loader: bool,
    recover: bool,
    wrong_firmware: Option<&'a String>,
    recovery_token: Option<&'a String>,
    confirm: bool,
}

fn cmd_flash(device: &str, args: FlashArgs<'_>) {
    // Validate conflicting options
    if args.encrypted && args.include_boot_loader {
        eprintln!("ERROR: --encrypted and --include-boot-loader cannot be combined");
        process::exit(1);
    }

    let firmware = match args.firmware {
        Some(f) => f.as_str(),
        None => {
            eprintln!("ERROR: -i <firmware> is required");
            process::exit(1);
        }
    };

    let firmware_data = match std::fs::read(firmware) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ERROR: cannot read firmware: {e}");
            process::exit(1);
        }
    };

    // Probe drive and determine safety
    let (backend, path) = find_backend();
    let info_cmd = command::plan_drive_info(backend, &path, device);
    let info_out = drive::run_command(&info_cmd.program, &info_cmd.args).unwrap_or_else(|e| {
        eprintln!("ERROR: cannot probe drive: {e}");
        process::exit(1);
    });

    let safety = command::classify_drive_safety(device, &info_out.combined());
    if !safety.mt1959 {
        eprintln!("ERROR: drive is not MT1959 platform");
        process::exit(1);
    }

    // Parse drive identity from info output for manifest matching
    let drive_match = parse_drive_identity(device, &info_out.combined());

    // If manifest provided, run validation
    if let Some(mp) = args.manifest_path {
        let manifest_data = match std::fs::read(mp) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("ERROR: cannot read manifest: {e}");
                process::exit(1);
            }
        };
        let manifest = match manifest::parse_manifest(&manifest_data) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("ERROR: invalid manifest: {e}");
                process::exit(1);
            }
        };

        // Resolve image ID
        let image_id = resolve_image_id(&manifest, args.image_id);

        let request = flash::FlashPlanRequest {
            image_id: &image_id,
            current_version: &drive_match.revision,
            firmware_size: firmware_data.len() as u64,
            firmware_sha256: &flash::sha256_hex(&firmware_data),
            signature_present: manifest
                .firmware_images
                .iter()
                .find(|i| i.image_id == image_id)
                .map(|i| i.signature_present)
                .unwrap_or(false),
            user_confirmed: args.confirm,
        };

        match flash::build_flash_plan(&manifest, &drive_match, request) {
            Ok(plan) => {
                let report = flash::dry_run(&plan);
                println!("{}", report.summary);
                if !report.would_execute {
                    if !args.confirm {
                        println!("Add --confirm to proceed.");
                    }
                    process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("Validation failed: {e}");
                process::exit(1);
            }
        }
    }

    // Build the operation
    let operation = if args.recover {
        let token = resolve_recovery_token(args.wrong_firmware, args.recovery_token);
        command::Operation::Recover {
            firmware_path: firmware.to_string(),
            recovery_boot_token: token,
        }
    } else {
        command::Operation::Write {
            firmware_path: firmware.to_string(),
            encrypted: args.encrypted,
            include_boot_loader: args.include_boot_loader,
        }
    };

    let req = command::PlanRequest {
        backend,
        tool_path: path,
        drive: device.to_string(),
        drive_is_mt1959: safety.mt1959,
        confirmation: if args.confirm {
            command::required_flash_confirmation(device)
        } else {
            String::new()
        },
        operation,
    };

    match command::plan_command(req) {
        Ok(plan) => {
            if !args.confirm {
                println!("Dry-run: {}", plan.command.program);
                println!("  args: {:?}", plan.command.args);
                println!("Add --confirm to proceed.");
                return;
            }
            match drive::run_command(&plan.command.program, &plan.command.args) {
                Ok(out) => {
                    if out.success() {
                        println!("Flash completed successfully.");
                    } else {
                        eprintln!("ERROR: {}", out.combined());
                        process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("ERROR: {e}");
                    process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Cannot plan flash: {e}");
            process::exit(1);
        }
    }
}

/// Parse drive identity from sdftool `--info` output for manifest matching.
fn parse_drive_identity(device: &str, info_output: &str) -> manifest::DriveMatch {
    let mut vendor = String::new();
    let mut model = String::new();
    let mut revision = String::new();

    for line in info_output.lines() {
        let line = line.trim();
        if let Some(val) = line
            .strip_prefix("Vendor:")
            .or_else(|| line.strip_prefix("vendor:"))
        {
            vendor = val.trim().to_string();
        } else if let Some(val) = line
            .strip_prefix("Product:")
            .or_else(|| line.strip_prefix("product:"))
            .or_else(|| line.strip_prefix("Model:"))
            .or_else(|| line.strip_prefix("model:"))
        {
            model = val.trim().to_string();
        } else if let Some(val) = line
            .strip_prefix("Revision:")
            .or_else(|| line.strip_prefix("revision:"))
            .or_else(|| line.strip_prefix("Firmware:"))
            .or_else(|| line.strip_prefix("firmware:"))
        {
            revision = val.trim().to_string();
        }
    }

    // Fall back to device label parsing if info output didn't contain fields
    if vendor.is_empty() && model.is_empty() {
        let parts: Vec<&str> = device.split(['_', '-', ' ']).collect();
        if parts.len() >= 2 {
            vendor = parts[0].to_string();
            model = parts[1].to_string();
        }
    }

    manifest::DriveMatch {
        vendor,
        model,
        revision,
    }
}

/// Resolve image ID from manifest, picking the only image if there's exactly one.
fn resolve_image_id(manifest: &manifest::FirmwareManifest, explicit: Option<&String>) -> String {
    if let Some(id) = explicit {
        return id.clone();
    }
    if manifest.firmware_images.len() == 1 {
        manifest.firmware_images[0].image_id.clone()
    } else {
        eprintln!(
            "ERROR: manifest contains {} images; specify --image-id",
            manifest.firmware_images.len()
        );
        for img in &manifest.firmware_images {
            eprintln!("  - {}", img.image_id);
        }
        process::exit(1);
    }
}

/// Resolve recovery boot token from either explicit flag or by extracting from a firmware file.
fn resolve_recovery_token(
    wrong_firmware: Option<&String>,
    explicit_token: Option<&String>,
) -> String {
    if let Some(token) = explicit_token {
        return token.clone();
    }
    if let Some(path) = wrong_firmware {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("ERROR: cannot read wrong firmware {path}: {e}");
                process::exit(1);
            }
        };
        match command::extract_recovery_boot_token(&data) {
            Ok(token) => {
                println!("Extracted recovery boot token: {token}");
                token
            }
            Err(e) => {
                eprintln!("ERROR: cannot extract recovery boot token: {e}");
                process::exit(1);
            }
        }
    } else {
        eprintln!("ERROR: --recover requires either --recovery-token or --wrong-firmware");
        process::exit(1);
    }
}

fn cmd_sdf_info(file: &str) {
    let data = match std::fs::read(file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ERROR: cannot read {file}: {e}");
            process::exit(1);
        }
    };

    let mut cursor = std::io::Cursor::new(&data);
    match sdf::parse_sdf0(&mut cursor) {
        Ok(container) => {
            println!("SDF0 Container: {file}");
            println!("  Version:        {}", container.header.version);
            println!("  Header size:    {}", container.header.header_size);
            println!("  Table offset:   {}", container.header.table_offset);
            println!("  Flags:          0x{:08x}", container.header.flags);
            println!("  Payload offset: {}", container.payload.offset);
            println!("  Encrypted:      {}", container.payload.encrypted);
            println!("  Compressed:     {}", container.payload.compressed);
            if let Some(v) = &container.metadata.vendor {
                println!("  Vendor:         {v}");
            }
            if let Some(m) = &container.metadata.model {
                println!("  Model:          {m}");
            }
            if let Some(fw) = &container.metadata.firmware_version {
                println!("  Firmware:       {fw}");
            }
            for (k, v) in &container.metadata.extra {
                println!("  {k}: {v}");
            }
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            process::exit(1);
        }
    }
}
