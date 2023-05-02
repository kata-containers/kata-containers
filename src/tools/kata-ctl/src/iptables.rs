// Copyright (c) 2023 Alec Pemberton, Juanaiga Okugas
//
// SPDX-License-Identifier: Apache-2.0
//

use clap::{App, Arg, Parser, SubCommand, Command, Parser};
use crate::args::{IptablesCommand};
use reqwest::{Url};
use std::{fs};
use anyhow::Result;
use shimclient::MgmtClient;
use args::{Commands};
use std::process::Command;
use thiserror::Error;

//kata-proxy management API endpoint, without code would not know the location of the unix sockets
const DEFAULT_TIMEOUT: u64 = 30;
const IP_TABLES_SOCKET: &str = "unix:///run/vc/sbs/{sandbox_id}/ip_tables";
const IP6_TABLES_SOCKET: &str = "unix:///run/vc/sbs/{sandbox_id}/ip6_tables";

#[derive(thiserror::Error, Debug)]
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

pub fn handle_iptables(_args: IptablesCommand) -> Result<()> {

    //implement handle_iptables
    // let args = KataCtlCli::parse();
    // match args.command{
    //     Commands::Iptables(args) => handle_iptables(args),
    // }

    let matches = Command::new("iptables")
    .subcommand(Command::new("get"))
    .subcommand(Command::new("set"))
    .get_matches();

    //checking for subcommand entered form user 
    match matches.subcommand() {
        Some(("get", get_matches)) => {
            // retrieve the sandbox ID from the command line arguments
            let sandbox_id = get_matches.value_of("sandbox-id").unwrap();
            // check if ipv6 is requested
            let is_ipv6 = get_matches.is_present("v6");
            // verify the container ID before proceeding
            verify_id(sandbox_id)?;//validate::verify_id(sandbox_id)
            // generate the appropriate URL for the iptables request to connect Kata to agent within guest
            let url = if is_ipv6 {
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
        Some(("set", set_matches)) => {
            // Extract sandbox ID and IPv6 flag from command-line arguments
            let sandbox_id = set_matches.value_of("sandbox-id").unwrap();
            let is_ipv6 = set_matches.is_present("v6");
            let iptables_file = set_matches.value_of("file").unwrap();
            
            // Verify the specified sandbox ID is valid
            verify_id(sandbox_id)?;//verify_container_id(sandbox_id)?;
        
            // Check if the iptables file was provided
            if iptables_file.is_empty() {
                return Err("iptables file not provided".into());
            }
            
            // Check if the iptables file exists
            if !std::path::Path::new(iptables_file).exists() {
                return Err(format!("iptables file does not exist: {}", iptables_file).into());
            }
        
            // Read the contents of the specified iptables file into a buffer
            let buf = fs::read(iptables_file)?;
        
            // Set the content type for the request
            let content_type = "application/octet-stream";
        
            // Determine the URL for the management API endpoint based on the IPv6 flag
            let url = if is_ipv6 {
                Url::parse(&format!("{}{}", IP6_TABLES_SOCKET, sandbox_id))?
            } else {
                Url::parse(&format!("{}{}", IP_TABLES_SOCKET, sandbox_id))?
            };
        
            // Create a new management client for the specified sandbox ID
            let shim_client = match MgmtClient::new(sandbox_id, Some(DEFAULT_TIMEOUT)) {
                Ok(client) => client,
                Err(e) => return Err(format!("Error creating management client: {}", e).into()),
            };
        
            // Send a PUT request to set the iptables rules
            let response = match shim_client.put(url, content_type, &buf) {
                Ok(res) => res,
                Err(e) => return Err(format!("Error sending request: {}", e).into()),
            };
        
            // Check if the request was successful
            if !response.status().is_success() {
                return Err(format!("Request failed with status code: {}", response.status()).into());
            }
        
            // Print a message indicating that the iptables rules were set successfully
            println!("iptables set successfully");
        
            // Return Ok to indicate success
            Ok(())
        }
        
    }

}