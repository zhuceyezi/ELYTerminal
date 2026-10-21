use std::{env, ffi::OsString, os::unix::ffi::OsStrExt, path::PathBuf};

use anyhow::{Context as _, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    settings::SettingsStore,
    term::{
        Launch, Quote,
        shell::{self, ShellIntegration},
    },
};

/// One external launch, with paths kept as Unix bytes on the wire.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub directory: Vec<u8>,
    pub command: Vec<String>,
    pub new_window: bool,
    pub activation_token: Option<String>,
}

impl Request {
    pub fn parse(arguments: Vec<OsString>, cwd: PathBuf) -> anyhow::Result<Self> {
        let mut args = arguments.into_iter();
        let mut directory = cwd.clone();
        let mut command = Vec::new();
        let mut new_window = false;
        while let Some(argument) = args.next() {
            let bytes = argument.as_bytes();
            if argument == "--new-window" {
                new_window = true;
            } else if matches!(
                bytes,
                b"--working-directory" | b"--workdir" | b"--directory"
            ) {
                directory = args
                    .next()
                    .context("startup: directory option needs a path")?
                    .into();
            } else if let Some(path) = [
                b"--working-directory=".as_slice(),
                b"--workdir=",
                b"--directory=",
            ]
            .into_iter()
            .find_map(|prefix| bytes.strip_prefix(prefix))
            {
                use std::os::unix::ffi::OsStringExt;
                ensure!(!path.is_empty(), "startup: directory option needs a path");
                directory = OsString::from_vec(path.to_vec()).into();
            } else if argument == "-e" || argument == "--command" || argument == "--" {
                command = args
                    .map(|arg| {
                        arg.into_string()
                            .map_err(|_| anyhow::anyhow!("startup: command is not UTF-8"))
                    })
                    .collect::<anyhow::Result<_>>()?;
                ensure!(
                    !command.is_empty(),
                    "startup: command option needs a program"
                );
                break;
            } else {
                bail!("startup: unknown option {argument:?}; use --help");
            }
        }
        if !directory.is_absolute() {
            directory = cwd.join(directory);
        }
        let request = Self {
            directory: directory.as_os_str().as_bytes().to_vec(),
            command,
            new_window,
            activation_token: env::var("XDG_ACTIVATION_TOKEN").ok(),
        };
        request.validate()?;
        Ok(request)
    }

    pub fn directory(&self) -> PathBuf {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(self.directory.clone()).into()
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let path = self.directory();
        ensure!(
            path.is_absolute() && path.is_dir(),
            "startup: not an existing absolute directory: {}",
            path.display()
        );
        ensure!(
            !self.directory.contains(&0),
            "startup: directory contains NUL"
        );
        ensure!(
            self.command.iter().all(|arg| !arg.contains('\0')),
            "startup: command contains NUL"
        );
        Ok(())
    }

    pub fn launch(&self, cx: &gpui::App) -> anyhow::Result<Launch> {
        self.validate()?;
        let mut launch = shell::launch(
            &SettingsStore::get(cx).shell,
            self.directory(),
            &cx.global::<ShellIntegration>().0,
        )?;
        if let Some(program) = self.command.first() {
            launch.program = Some(if self.command.len() == 1 {
                ("/bin/sh".into(), vec!["-c".into(), program.clone()])
            } else {
                (program.clone(), self.command[1..].to_vec())
            });
            launch.env.clear();
            launch.reports = false;
            launch.quote = Quote::Posix;
        }
        Ok(launch)
    }
}

#[cfg(test)]
mod tests;
