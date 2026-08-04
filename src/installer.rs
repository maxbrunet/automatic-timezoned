use std::{
    error::Error, fs, io::Write, os::unix::fs::PermissionsExt, path::Path, process::Command,
};

// Note: this is written to disk only for the purpose of uninstallation, so this just lists all files (and folders) that are likely installed (aka all the path that should be deleted when uninstalled). The first line denotes the manifest version, if future versions require more sophisticated uninstalls then this is how that will be detected
static INSTALLATION_MANIFEST: &str = r#"version: 1
/usr/bin/automatic-timezoned
/etc/geoclue/conf.d/automatic-timezoned.conf
/etc/polkit-1/rules.d/automatic-timezoned.rules
/etc/systemd/system/automatic-timezoned.service
/var/lib/automatic-timezoned/installation_manifest.txt
/var/lib/automatic-timezoned
"#;

static GEOCLUE_CONF_FILE: &str = r#"; vim: ft=dosini
[automatic-timezoned]
allowed=true
system=true
users={}
"#;

static POLKIT_CONF_FILE: &str = r#"// vim: ft=javascript
polkit.addRule(function(action, subject) {
  if (action.id == "org.freedesktop.timedate1.set-timezone" && subject.user == "{}") {
	return polkit.Result.YES;
  }
});
"#;

static SYSTEMD_SERVICE_FILE: &str = r#"# vim: ft=systemd
[Unit]
Description=Automatically update system timezone based on location

[Service]
User={}
ExecStart=/usr/bin/automatic-timezoned
Restart=on-failure
RestartSec=300

[Install]
WantedBy=multi-user.target
"#;

fn read_stdin() -> Result<String, Box<dyn Error>> {
    let mut output = String::new();
    std::io::stdout().flush()?;
    std::io::stdin().read_line(&mut output)?;
    Ok(output.trim().to_string())
}

fn get_stdin_yes_no() -> Result<bool, Box<dyn Error>> {
    loop {
        match read_stdin()?.as_str() {
            "Y" | "y" | "" => return Ok(true),
            "N" | "n" => return Ok(false),
            _ => print!("Please enter 'y' or 'n' "),
        }
    }
}

fn format_template(template: &str, args: &[&str]) -> String {
    let mut output = String::with_capacity(template.len());
    let mut parts = template.split("{}");

    output.push_str(parts.next().unwrap());

    let parts = parts.collect::<Vec<_>>();
    debug_assert_eq!(args.len(), parts.len());
    for (arg, part) in args.iter().zip(parts) {
        output.push_str(arg);
        output.push_str(part);
    }

    output
}

fn run_cmd(cmd: &str, args: &[&str]) -> Result<(), Box<dyn Error>> {
    let status = Command::new(cmd).args(args).status()?;
    if !status.success() {
        return Err(Box::new(std::io::Error::other(format!(
            "Failed to execute command {cmd} {args:?}, error code {:?}",
            status.code()
        ))));
    }
    Ok(())
}

fn safe_write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<(), Box<dyn Error>> {
    let (path, contents) = (path.as_ref(), contents.as_ref());
    let output_folder = path.parent().ok_or_else(|| {
        Box::new(std::io::Error::other(format!(
            "Failed to write file, output path has no parent. Failed path: {path:?}"
        )))
    })?;
    fs::create_dir_all(output_folder)?;
    let mut temp_file = tempfile::NamedTempFile::new_in(output_folder)?;
    temp_file.write_all(contents)?;
    temp_file.persist(path)?;
    Ok(())
}

pub fn install() -> Result<(), Box<dyn Error>> {
    // step 1: test if this is root
    if !nix::unistd::geteuid().is_root() {
        println!(
            "This must be run as root, please rerun with `sudo ./automatic-timezoned --install`"
        );
        return Ok(());
    }

    println!("Installing Automatic Timezoned");
    println!("Warning: this installer is still experimental and you might still need to manually configure your system after installation");

    // step 2: get user name and uid
    let uid = std::env::var("SUDO_UID")?;
    let uid: u32 = uid.parse()?;
    println!("Detected user uid is {uid}");
    let user_name = std::env::var("SUDO_USER")?;
    println!("Detected user name is {user_name}");

    // step 3: write installation manifest
    fs::create_dir_all("/var/lib/automatic-timezoned")?;
    fs::write(
        "/var/lib/automatic-timezoned/installation_manifest.txt",
        INSTALLATION_MANIFEST,
    )?;
    println!("Wrote installation manifest");

    let mut did_all_install = true;

    // step 4: copy binary
    print!("Install (copy) binary to `/usr/bin/automatic-timezoned`? Y/n ");
    if get_stdin_yes_no()? {
        fs::create_dir_all("/usr/bin")?;
        fs::copy(std::env::current_exe()?, "/usr/bin/automatic-timezoned")?;
        fs::set_permissions(
            "/usr/bin/automatic-timezoned",
            fs::Permissions::from_mode(0o755),
        )?;
    } else {
        println!("Skipped installing binary");
        did_all_install = false;
    }

    // step 5: write geoclue config file
    print!("Install geoclue config file to `/etc/geoclue/conf.d/automatic-timezoned.conf`? Y/n ");
    if get_stdin_yes_no()? {
        let geoclue_conf_file = format_template(GEOCLUE_CONF_FILE, &[uid.to_string().as_str()]);
        safe_write(
            "/etc/geoclue/conf.d/automatic-timezoned.conf",
            geoclue_conf_file,
        )?;
        fs::set_permissions(
            "/etc/geoclue/conf.d/automatic-timezoned.conf",
            fs::Permissions::from_mode(0o644),
        )?;
    } else {
        println!("Skipped installing geoclue config file");
        did_all_install = false;
    }

    // step 6: write polkit config file
    print!("Install polkit config file to `/etc/polkit-1/rules.d/automatic-timezoned.rules`? Y/n ");
    if get_stdin_yes_no()? {
        let polkit_conf_file = format_template(POLKIT_CONF_FILE, &[user_name.as_str()]);
        safe_write(
            "/etc/polkit-1/rules.d/automatic-timezoned.rules",
            polkit_conf_file,
        )?;
        fs::set_permissions(
            "/etc/polkit-1/rules.d/automatic-timezoned.rules",
            fs::Permissions::from_mode(0o644),
        )?;
    } else {
        println!("Skipped installing polkit config file");
        did_all_install = false;
    }

    // step 7: write systemd service file
    print!(
        "Install systemd service file to `/etc/systemd/system/automatic-timezoned.service`? Y/n "
    );
    if get_stdin_yes_no()? {
        let systemd_service_file = format_template(SYSTEMD_SERVICE_FILE, &[user_name.as_str()]);
        safe_write(
            "/etc/systemd/system/automatic-timezoned.service",
            systemd_service_file,
        )?;
        fs::set_permissions(
            "/etc/systemd/system/automatic-timezoned.service",
            fs::Permissions::from_mode(0o644),
        )?;
    } else {
        println!("Skipped installing systemd service file");
        did_all_install = false;
    }

    // step 7: run `systemctl daemon-reload`
    run_cmd("systemctl", &["daemon-reload"])?;
    println!("Ran `systemctl daemon-reload`");

    // step 8: enable systemd service
    if did_all_install {
        print!("Start systemd service? Y/n ");
        if get_stdin_yes_no()? {
            run_cmd(
                "systemctl",
                &["enable", "--now", "automatic-timezoned.service"],
            )?;
        } else {
            println!("Skipped starting systemd service");
            did_all_install = false;
        }
    } else {
        println!("Because some install options were skipped, the service will not be started");
    }

    // step 9: check systemd service
    println!("Checking systemd service status:");
    // not using run_cmd() here because a failure here shouldn't stop the program
    let status = Command::new("systemctl")
        .args(["--no-pager", "status", "automatic-timezoned.service"])
        .status()?;
    if !status.success() {
        println!(
            "Warning: systemctl status failed with exit code {:?}",
            status.code()
        );
    }

    println!("Finished installation, please check systemd status above.");
    if !did_all_install {
        println!("Warning: some installation were skipped and the program may not be fully installed, if you want to reinstall this then please first run `sudo ./automatic-timezoned --uninstall`");
    }

    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn Error>> {
    // step 1: test if this is root
    if !nix::unistd::geteuid().is_root() {
        println!(
            "This must be run as root, please rerun with `sudo ./automatic-timezoned --uninstall`"
        );
        return Ok(());
    }

    // step 2: get manifest and run appropriate installer
    if !fs::exists("/var/lib/automatic-timezoned/installation_manifest.txt")? {
        eprintln!("Fatal: could not find the essential installation manifest file, so the program cannot be uninstalled.");
        return Ok(());
    }

    let manifest = fs::read_to_string("/var/lib/automatic-timezoned/installation_manifest.txt")?;
    let version = match manifest.lines().next() {
        Some(v) => v,
        None => {
            return Err(Box::new(std::io::Error::other(
                "Cannot uninstall because installation manifest appears to be empty",
            )))
        }
    };
    let version = match version.strip_prefix("version: ") {
		Some(v) => v,
		None => return Err(Box::new(std::io::Error::other("Cannot uninstall because installation manifest is malformed, must start with \"version: {}\""))),
	};

    match version {
        "1" => uninstall_v1(manifest)?,
        _ => {
            return Err(Box::new(std::io::Error::other(
                "Cannot uninstall because the installation manifest version is unknown",
            )))
        }
    }

    println!("Uninstallation is complete");

    Ok(())
}

fn uninstall_v1(manifest: String) -> Result<(), Box<dyn Error>> {
    // step 1: remove systemd service
    println!("Disabling systemd service...");
    let result = run_cmd(
        "systemctl",
        &["disable", "--now", "automatic-timezoned.service"],
    );
    if let Err(err) = result {
        eprintln!("WARNING: Failed to disable systemd service: {err}");
    }

    // step 2: remove all files and folders listed in installation manifest
    let installed_files = manifest
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    let mut skipped_files = vec![];
    for path in installed_files {
        if !fs::exists(path)? {
            eprintln!("WARNING: could not find the file \"{path}\" and it will not be automatically removed (the user might have installed it somewhere else?)");
            skipped_files.push(path);
            continue;
        }
        let is_dir = Path::new(path).is_dir();
        print!(
            "Remove {} \"{path}\"? Y/n ",
            if is_dir { "folder" } else { "file" }
        );
        if get_stdin_yes_no()? {
            if is_dir {
                fs::remove_dir(path)?;
            } else {
                fs::remove_file(path)?;
            }
        } else {
            skipped_files.push(path);
        }
    }

    // step 3: run `systemctl daemon-reload`
    run_cmd("systemctl", &["daemon-reload"])?;
    println!("Ran `systemctl daemon-reload`");

    // final warnings
    if !skipped_files.is_empty() {
        eprintln!("Warning: these files were skipped during uninstallation and could still be on your computer:");
        for path in skipped_files {
            eprintln!("{path}");
        }
    }

    Ok(())
}
