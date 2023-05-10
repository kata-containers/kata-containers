// Copyright (c) 2023 Alec Pemberton, Juanaiga Okugas
//
// SPDX-License-Identifier: Apache-2.0
//

//use clap::{App, arg, Parser, SubCommand, Command};
use reqwest::{Url};
use std::{fs};
use anyhow::{Result, Context};//Context
use shim_interface::shim_mgmt::client::MgmtClient;
use crate::args::{IptablesCommand, IpTablesArguments};//Commands
use thiserror::Error;
use std::time::Duration;
//use super::*;

//kata-proxy management API endpoint, without code would not know the location of the unix sockets
const DEFAULT_TIMEOUT: u64 = 30;
const IP_TABLES_SOCKET: &str = "unix:///run/vc/sbs/{sandbox_id}/ip_tables";
const IP6_TABLES_SOCKET: &str = "unix:///run/vc/sbs/{sandbox_id}/ip6_tables";

#[derive(Error, Debug)]
pub enum Error{
    #[error("Invalid Container ID {0}")]
    InvalidContainerID(String),
}

//Verify Id for validating sandboxID
pub fn verify_id(id:&str) -> Result<(), Error>{
    let mut chars = id.chars();

    let valid = matches!(chars.next(), Some(first) if first.is_alphanumeric()
    &&id.len() >1
    && chars.all(|c| c.is_alphanumeric() || ['.', '-', '_'].contains(&c)));

    match valid {
        true => Ok(()),
        false => Err(Error::InvalidContainerID(id.to_string())),
    }
}

pub async fn handle_iptables(args: IptablesCommand) -> Result<(), anyhow::Error> {//pub fn handle
    //checking for subcommand entered form user 
    match args.subcommand() {//.subcommand()
        IpTablesArguments::Get{sandbox_id, v6} =>{//Some(("get", get_matches)) => {
            // retrieve the sandbox ID from the command line arguments
            let sandbox_id = sandbox_id;//get_matches.value_of("sandbox-id")?;

            let is_ipv6 = v6;//v6;get_matches.is_present("v6");
           
            verify_id(sandbox_id)?;
            // generate the appropriate URL for the iptables request to connect Kata to agent within guest
            let url = if *is_ipv6 {
                Url::parse(&format!("{}{}", IP6_TABLES_SOCKET, sandbox_id))?
            } else {
                Url::parse(&format!("{}{}", IP_TABLES_SOCKET, sandbox_id))?
            };
            // create a new management client for the specified sandbox ID
            let timeout = Duration::from_secs(DEFAULT_TIMEOUT);
            let shim_client = MgmtClient::new(sandbox_id, Some(timeout))?;
            
            // make the GET request to retrieve the iptables
            let mut response = shim_client.get(url.as_str()).await?;
            let body_bytes = hyper::body::to_bytes(response.body_mut()).await?;
	    let _body_str = std::str::from_utf8(&body_bytes)?;
            // Return an `Ok` value indicating success.
            Ok(())
        }
        IpTablesArguments::Set {sandbox_id, v6, file} => {//Some(("set", set_matches)) => {
            // Extract sandbox ID and IPv6 flag from command-line arguments
            let sandbox_id = sandbox_id;//set_matches.value_of("sandbox-id")?;
            let is_ipv6 = v6;//set_matches.is_present("v6");
            let iptables_file = file;//set_matches.value_of("file")?;
            
            // Verify the specified sandbox ID is valid
            verify_id(sandbox_id)?;
        
            // Read the contents of the specified iptables file into a buffer
            let buf = fs::read(iptables_file).map_err(|err| anyhow::Error::msg(format!("iptables file not provided: {}", err)))?;

            // Set the content type for the request
            let _content_type = "application/octet-stream";
        
            // Determine the URL for the management API endpoint based on the IPv6 flag
            let url = if *is_ipv6 {
                Url::parse(&format!("{}{}", IP6_TABLES_SOCKET, sandbox_id))?
            } else {
                Url::parse(&format!("{}{}", IP_TABLES_SOCKET, sandbox_id))?
            };

            // Create a new management client for the specified sandbox ID
	    let timeout = Duration::from_secs(DEFAULT_TIMEOUT);
            let shim_client = MgmtClient::new(sandbox_id, Some(timeout)).context("error creating management client")?;
            //     Ok(client) => client,
            //     Err(err) => return Err(err.into()),
            // };
        
            // Send a PUT request to set the iptables rules
            let response = shim_client.put(url.as_str(), buf).await.context("error sending request")?;//content_type
            //     Ok(res) => res,
            // };
        
            // Check if the request was successful
            if !response.status().is_success() {
                let status = response.status();
                let _body = format!("{:?}", response.into_body());
                return Err(anyhow::Error::msg(format!("Request failed with status code: {}", status)));
            }
        
            // Print a message indicating that the iptables rules were set successfully
            println!("iptables set successfully");
        
            Ok(())
        }
    }
}

//unit tests 
#[test]
fn test_verify_id(){
    assert!(verify_id("aasdf").is_ok());
    assert!(verify_id("aas-df").is_ok());
    assert!(verify_id("ABC.asdf02").is_ok());
    assert!(verify_id("a123af_01").is_ok());
    assert!(verify_id("123_ABC.def-456").is_ok());
}

#[test]
fn test_invalid_verify_id(){
    //invalid
    assert!(verify_id("").is_err());
    assert!(verify_id("#invalid").is_err());
    assert!(verify_id("a").is_err());
    assert!(verify_id("a**dd").is_err());
    assert!(verify_id("%invalid/id").is_err());
    assert!(verify_id("add-").is_err());
    assert!(verify_id("a<bb").is_err());
    assert!(verify_id("ad?blocker").is_err());
}
#[test]
fn test_handle_iptables_get_valid() {
    let args = IptablesCommand {
        command: Commands::Get,
        sandbox_id: "abc123".to_string(),
        v6: false,
        file: "/path/to/iptables".to_string(),
    };
    assert!(handle_iptables(args).is_ok());
}

#[test]
fn test_handle_iptables_get_invalid() {
    let args = IptablesCommand {
        command: Commands::Get,
        sandbox_id: "abc$123".to_string(),
        v6: false,
        file: "/path/to/iptables".to_string(),
    };
    assert!(handle_iptables(args).is_err());
}

#[test]
fn test_handle_iptables_set_valid() {
    let args = IptablesCommand {
        command: Commands::Set,
        sandbox_id: "abc123".to_string(),
        v6: false,
        file: "/path/to/iptables".to_string(),
    };
    assert!(handle_iptables(args).is_ok());
}

#[test]
fn test_handle_iptables_set_invalid() {
    let args = IptablesCommand {
        command: Commands::Set,
        sandbox_id: "abc$123".to_string(),
        v6: false,
        file: "/path/to/iptables".to_string(),
    };
    assert!(handle_iptables(args).is_err());
}

//get and set invalid sandbox id
#[test]
fn test_handle_iptables_get_invalid_sandboxid() {
    let args = IptablesCommand{
        command: Commands::Get,
        sandbox_id: "invalid_sandboxid".to_string(),
        v6: false,
        file: "/path/to/iptables".to_string(),
    };
    assert!(handle_iptables(args).is_err());
}

#[test]
fn test_handle_iptables_set_invalid_sandboxid() {
    let args = IptablesCommand{
        command: Commands::Set,
        sandbox_id: "invalid_sandboxid".to_string(),
        v6: false,
        file: "/path/to/iptables".to_string(),
    };
    assert!(handle_iptables(args).is_err());
}

//check for invalid file
#[test]
fn test_invalid_iptables_file(){
    let iptables_text = "check iptables";
    let iptables_File = tempfile::NamedTempFile::new()?;
    fs::write(iptables_file.path(), iptables_text)?;

    //Call set subcommand in handle_iptables
    let args = IptablesCommand::from_iter_safe(&[
        "iptables",
        "set",
        "--sandox-id",
        "1234",
        "--file",
        iptables_file.path().to_str()?,
    ])?;
    assert!(handle_iptables(args).is_ok());

    //Call get subcommand in handle_iptables
    let args = IptablesCommand::from_iter_safe(&[
        "iptables",
        "get",
        "--sandox-id",
        "1234",
        "--file",
        iptables_file.path().to_str()?,
    ])?;
    assert!(handle_iptables(args).is_ok());
}