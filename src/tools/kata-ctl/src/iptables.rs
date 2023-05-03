// Copyright (c) 2023 Alec Pemberton, Juanaiga Okugas
//
// SPDX-License-Identifier: Apache-2.0
//

//use clap::{App, arg, Parser, SubCommand, Command};
use reqwest::{Url};
use std::{fs};
use anyhow::{Result};//Context
use crate::shim_mgmt::client::MgmtClient;
use crate::args::{IptablesCommand, IpTablesArguments};//Commands
use thiserror::Error;
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

pub fn handle_iptables(args: IptablesCommand) -> Result<(), anyhow::Error> {
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
            let shim_client = MgmtClient::new(sandbox_id, Some(DEFAULT_TIMEOUT))?;
            // make the GET request to retrieve the iptables
            let response = shim_client.get(url)?;
            let body = response.text()?;
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
            let content_type = "application/octet-stream";
        
            // Determine the URL for the management API endpoint based on the IPv6 flag
            let url = if *is_ipv6 {
                Url::parse(&format!("{}{}", IP6_TABLES_SOCKET, sandbox_id))?
            } else {
                Url::parse(&format!("{}{}", IP_TABLES_SOCKET, sandbox_id))?
            };

            // Create a new management client for the specified sandbox ID
            let shim_client = MgmtClient::new(sandbox_id, Some(DEFAULT_TIMEOUT)).context("error creating management client")?;
            //     Ok(client) => client,
            //     Err(err) => return Err(err.into()),
            // };
        
            // Send a PUT request to set the iptables rules
            let response = shim_client.put(url, content_type, &buf).context("error sending request")?;
            //     Ok(res) => res,
            // };
        
            // Check if the request was successful
            if !response.status().is_success() {
                let body = format!("{:?}", response.into_body());
                return Err(anyhow::Error::msg(format!("Request failed with status code: {}", response.status())));
            }
        
            // Print a message indicating that the iptables rules were set successfully
            println!("iptables set successfully");
        
            // Return Ok to indicate success
            Ok(())
        }
    }
}

//Still a work in progress for the unit tests
//Unit tests
#[test]
fn test_verify_id_valid() {
    let result = verify_id("abc123");
    assert!(result.is_ok());
}

#[test]
fn test_verify_id_invalid() {
    let result = verify_id("123!abc");
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "Invalid Container ID 123!abc");
}

#[test]
fn test_handle_iptables_set_valid() {
    let args = IptablesCommand {
        command: Commands::Set,
        sandbox_id: "abc123".to_string(),
        v6: false,
        file: "/path/to/iptables".to_string(),
    };
    let result = handle_iptables(args);
    assert!(result.is_ok());
}

#[test]
fn test_handle_iptables_set_invalid() {
    let args = IptablesCommand {
        command: Commands::Set,
        sandbox_id: "123!abc".to_string(),
        v6: false,
        file: "/path/to/iptables".to_string(),
    };
    let result = handle_iptables(args);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "Invalid Container ID 123!abc");
}

#[test]
fn test_handle_iptables_get_valid() {
    let args = IptablesCommand {
        command: Commands::Get,
        sandbox_id: "abc123".to_string(),
        v6: false,
        file: "/path/to/iptables".to_string(),
    };
    let result = handle_iptables(args);
    assert!(result.is_ok());
}

#[test]
fn test_handle_iptables_get_invalid() {
    let args = IptablesCommand {
        command: Commands::Get,
        sandbox_id: "123!abc".to_string(),
        v6: false,
        file: "/path/to/iptables".to_string(),
    };
    let result = handle_iptables(args);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "Invalid Container ID 123!abc");
}