// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use serde_json;
use reqwest;

use crate::model::*;
use crate::error::*;
use crate::cli::Args;
use anyhow::bail;
use anyhow::Result;
use serde_json::Value;

pub fn check_versions(key: &str, version: &Value, args: &Args) -> Result<Vec<CheckResult>> {
    let mut results: Vec<CheckResult> = Vec::new();
    check_versions_recursive(key, version, args, &mut results)?;
    Ok(results)
}

pub fn check_versions_recursive(key: &str, versions: &Value, args: &Args, results: &mut Vec<CheckResult>) -> Result<()> {
    match &versions {
        Value::Null => {
            // Nothing to do, this value doesn't contain anything useful
            if args.verbose {
                println!("Value is Null");
            }
        },
        Value::Bool(value) => {
            // Nothing to do, this value doesn't contain anything useful
            if args.verbose {
                println!("Value is Bool({})", value);
            }
        },
        Value::Number(value) => {
            // Nothing to do, this value doesn't contain anything useful
            if args.verbose {
                println!("Value is Number({})", value);
            }
        },
        Value::String(value) => {
            // Nothing to do, this value doesn't contain anything useful
            if args.verbose {
                println!("Value is String({})", value);
            }
        },
        Value::Array(value) => {
            // Recurse into array elements
            if args.verbose {
                println!("Value is Array({:?})", value);
            }

            for item in value.iter() {
                // Use the key from this level
                check_versions_recursive(key, item, args, results)?;
            }
        },
        Value::Object(value) => {
            if args.verbose {
                println!("Value is Object({:?})", value);
            }

            let project = parse_project_from_value(versions);

            match project {
                Ok(project) => {
                    // Found a versioned item "Project" - check its version
                    if args.verbose {
                        println!("Value is Project({:?})", project);
                    }

                    let check_result = match check_project_version(&project, key, args) {
                        Ok(cresult) => cresult,
                        Err(err_str) => CheckResult {
                            project_name: String::from(key),
                            current_version: String::from("unknown"),
                            latest_version: String::from("unknown"),
                            up_to_date: false,
                            success: false,
                            message: Some(err_str.to_string())
                        }
                    };

                    results.push(check_result);
                }
                Err(_) => {
                    // Not a project - recurse into object elements
                    if args.verbose {
                        println!("Value is not a Project");
                    }

                    for (subkey, value) in value.iter() {
                        if args.verbose {
                            println!("Recursing into subkey={}", subkey);
                        }

                        check_versions_recursive(&format!("{}.{}", key, subkey), value, args, results)?;
                    }
                }
            }
        },
    }

    Ok(())
}

fn parse_project_from_value(value: &Value) -> Result<Project> {
    let project = serde_json::from_value::<Project>(value.clone());

    match project {
        Ok(project) => {
            if project.url.is_none() && project.version.is_none() && project.tag.is_none() && project.branch.is_none() {
                bail!("Value parsed as project but has no url, version, tag, or branch. Probably an intermediate level")
            } else {
                Ok(project)
            }
        }
        Err(err) => {
            bail!(err)
        }
    }
}

fn check_project_version(project: &Project, name: &str, args: &Args) -> Result<CheckResult> {
    let current_version = match get_version_string(name, &project) {
        Ok(version) => version,
        Err(_e) => {
            String::from("unknown")
        }
    };

    if let Some(architectures) = &project.architecture {
        for (arch_name, _arch_value) in architectures.iter() {
            if args.verbose {
                println!("project: {}.{}, Architectures not implemented yet", name, arch_name);
            }
        }
        bail!("Architectures are not supported, yet")
    } else {
        match &project.url {
            Some(url) => {
                if is_github_url(url.as_str()) {
                   check_github_version(url.as_str(), current_version.as_str(), name, &args)
                } else {
                    match name {
                        "root.externals.virtiofsd" => check_virtiofsd_version(name, current_version.as_str()),
                        _ => bail!("Unknown url format")
                    }
                }
            },
            None => {
                // Assume project is a language if url is not present
                check_language_version(name, current_version.as_str(), &args)
            }
        }
    }
}

fn check_language_version(
    name: &str,
    current_version: &str,
    args: &Args) -> Result<CheckResult> {
    match name {
        "root.languages.golang" => {
            let url = "https://golang.org/VERSION?m=text";
            match get_go_latest_version(url) {
                Ok(latest_version) => {
                    let up_to_date = current_version.eq(&latest_version);
                    let check_result = CheckResult {
                        project_name: String::from(name),
                        current_version: String::from(current_version),
                        latest_version: String::from(latest_version),
                        up_to_date: up_to_date,
                        success: true,
                        message: None
                    };

                    Ok(check_result)
                },
                Err(_e) => {
                    bail!("Failed to check version")
                }
            }
        },
        "root.languages.golangci-lint" => {
            let url = "https://github.com/golangci/golangci-lint";
            match get_github_latest_version(url, &args) {
                Ok(latest_version) => {
                    let up_to_date = current_version.eq(&latest_version);
                    let check_result = CheckResult {
                        project_name: String::from(name),
                        current_version: String::from(current_version),
                        latest_version: String::from(latest_version),
                        up_to_date: up_to_date,
                        success: true,
                        message: None
                    };

                    Ok(check_result)
                },
                Err(_e) => {
                    bail!("Failed to check version")
                }
            }
        },
        "root.languages.rust" => {
            let url = "https://api.github.com/repos/rust-lang/rust/releases/latest";
            match get_rust_latest_version(url, &args) {
                Ok(latest_version) => {
                    let up_to_date = current_version.eq(&latest_version);
                    let check_result = CheckResult {
                        project_name: String::from(name),
                        current_version: String::from(current_version),
                        latest_version: String::from(latest_version),
                        up_to_date: up_to_date,
                        success: true,
                        message: None
                    };

                    Ok(check_result)
                },
                Err(_e) => {
                    bail!("Failed to check version")
                }
            }
        },
        _ => bail!("Unknown Language. Failed to check version")
    }
}

fn get_version_string(key: &str, project: &Project) -> Result<String> {
    match &project.tag {
        Some(tag) => Ok(tag.clone()),
        None => match &project.branch {
            Some(branch) => Ok(branch.clone()),
            None => match &project.version {
                Some(version) => Ok(version.clone()),
                None => bail!("Project {} is missing version (no version, branch, or tag)", key)
            }
        }
    }
}

fn get_github_latest_version(url: &str, args: &Args) -> Result<String> {
    let github_url = to_github_api_url(url);
    let mut client = reqwest::blocking::Client::new()
        .get(github_url)
        .header("User-Agent", "Check Versions v1.0");

    match &args.github_token {
        Some(github_token) => {
            if !github_token.is_empty() {
                client = client.header("Authorization", "Bearer ".to_owned() + github_token)
            }
        },
        None => ()
    }

    let versions_response = client.send()?.text()?;
    let versions: serde_json::Value = serde_json::from_str(versions_response.as_str())?;

    let tag = versions.get("tag_name")
        .ok_or(Box::new(ParserError {}))?
        .as_str()
        .ok_or(Box::new(ParserError {}))?;
    Ok(String::from(tag))
}

fn check_github_version(
    url: &str,
    current_version: &str,
    name: &str,
    args: &Args) -> Result<CheckResult> {
    match get_github_latest_version(url, &args) {
        Ok(latest_version) => {
            let up_to_date = current_version.eq(&latest_version);
            let check_result = CheckResult {
                project_name: String::from(name),
                current_version: String::from(current_version),
                latest_version: String::from(latest_version),
                up_to_date: up_to_date,
                success: true,
                message: None
            };

            Ok(check_result)
        },
        Err(_e) => {
            bail!("Failed to check version")
        }
    }
}

fn check_virtiofsd_version(
    name: &str,
    current_version: &str) -> Result<CheckResult> {
    let url = "https://gitlab.com/api/v4/projects/21523468/repository/tags";
    match get_virtiofsd_latest_version(url) {
        Ok(latest_version) => {
            let up_to_date = current_version.eq(&latest_version);
            let check_result = CheckResult {
                project_name: String::from(name),
                current_version: String::from(current_version),
                latest_version: String::from(latest_version),
                up_to_date: up_to_date,
                success: true,
                message: None
            };

            Ok(check_result)
        },
        Err(_e) => {
            bail!("Failed to check version")
        }
    }
}

fn get_rust_latest_version(url: &str, args: &Args) -> Result<String> {
    let mut client = reqwest::blocking::Client::new()
        .get(url)
        .header("User-Agent", "Check Versions v1.0");

    match &args.github_token {
        Some(github_token) => if !github_token.is_empty() {client = client.header("Authorization", "Bearer ".to_owned() + github_token)},
        None => ()
    }

    let versions_response = client.send()?.text()?;
    let versions: serde_json::Value = serde_json::from_str(versions_response.as_str())?;

    let tag = versions.get("tag_name")
        .ok_or(Box::new(ParserError {}))?
        .as_str()
        .ok_or(Box::new(ParserError {}))?;
    Ok(String::from(tag))
}

fn get_virtiofsd_latest_version(url: &str) -> Result<String> {
    let client = reqwest::blocking::Client::new()
        .get(url)
        .header("User-Agent", "Check Versions v1.0");

    let versions_response = client.send()?.text()?;
    let versions: serde_json::Value = serde_json::from_str(versions_response.as_str())?;

    let tag = versions.get(0)
        .ok_or(Box::new(ParserError {}))?
        .get("name")
        .ok_or(Box::new(ParserError {}))?
        .as_str()
        .ok_or(Box::new(ParserError {}))?;
    Ok(String::from(tag))
}


fn get_go_latest_version(url: &str) -> Result<String> {
    let version_response = reqwest::blocking::Client::new()
                            .get(url).send()?.text()?;

    let version = match version_response.split_once('\n') {
        Some(version_tuple) => String::from(version_tuple.0),
        None => String::from("unknown")
    };

    Ok(version)
}

fn to_github_api_url(url: &str) -> String {
    match url {
        x if x.contains("runtime-spec") => {
            return  url
                .replace("https://github.com", "https://api.github.com/repos")
                .replace("releases", "releases/latest")
                .to_string();
        },
        x if x.contains("containerd/containerd") =>{
            return (url
                .replace("github.com", "https://api.github.com/repos") + "/releases/latest")
                .to_string();
        },
        _ => {
            return (url
                .replace("https://github.com", "https://api.github.com/repos") + "/releases/latest")
                .to_string();
        }
    }
}

fn is_github_url(url: &str) -> bool {
    url.contains("github.com")
}
