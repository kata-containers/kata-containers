use clap::{App, Arg, SubCommand};
use reqwest::{Url};
use std::{fs, process};
use shimclient::MgmtClient;

//kata-proxy management API endpoint, without code would not know the location of the unix sockets
const DEFAULT_TIMEOUT: u64 = 30;
const IP_TABLES_SOCKET: &str = "unix:///run/vc/sbs/{sandbox_id}/ip_tables";
const IP6_TABLES_SOCKET: &str = "unix:///run/vc/sbs/{sandbox_id}/ip6_tables";

//main function for error handeling
fn main() {
    if let Err(e) = new_main() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

// Define the function signature. It returns `Result<(), Box<dyn std::error::Error>>`,
// which means it can either return an `Ok(())` value indicating success or an `Err` value
// containing a boxed error type that implements the `std::error::Error` trait.
fn new_main() -> Result<(), Box<dyn std::error::Error>> {

    // Define the command line interface using the `clap` library.
    let matches = App::new("kata-iptables") // Set the name of the program.
        .about("Get or set iptables within the Kata Containers guest") // Set a description of the program.
        .subcommand(
            SubCommand::with_name("get") // Add a subcommand named "get".
                .about("Get iptables from the Kata Containers guest") // Set a description of the "get" subcommand.
                .arg(
                    Arg::with_name("sandbox-id") // Add an argument named "sandbox-id".
                        .long("sandbox-id") // Set the long-form flag name for this argument.
                        .value_name("ID") // Set the value name that will be shown in the help message.
                        .required(true) // Indicate that this argument is required.
                        .takes_value(true) // Indicate that this argument takes a value.
                        .help("The target sandbox for getting the iptables"), // Set a description of this argument.
                )
                .arg(
                    Arg::with_name("v6") // Add an argument named "v6".
                        .long("v6") // Set the long-form flag name for this argument.
                        .help("Indicate we're requesting ipv6 iptables"), // Set a description of this argument.
                ),
        )
        .subcommand(
            SubCommand::with_name("set") // Add a subcommand named "set".
                .about("Set iptables in a specific Kata Containers guest based on file") // Set a description of the "set" subcommand.
                .arg(
                    Arg::with_name("sandbox-id") // Add an argument named "sandbox-id".
                        .long("sandbox-id") // Set the long-form flag name for this argument.
                        .value_name("ID") // Set the value name that will be shown in the help message.
                        .required(true) // Indicate that this argument is required.
                        .takes_value(true) // Indicate that this argument takes a value.
                        .help("The target sandbox for setting the iptables"), // Set a description of this argument.
                )
                .arg(
                    Arg::with_name("v6") // Add an argument named "v6".
                        .long("v6") // Set the long-form flag name for this argument.
                        .help("Indicate we're requesting ipv6 iptables"), // Set a description of this argument.
                )
                .arg(
                    Arg::with_name("file") // Add an argument named "file".
                        .value_name("FILE") // Set the value name that will be shown in the help message.
                        .required(true) // Indicate that this argument is required.
                        .takes_value(true) // Indicate that this argument takes a value.
                        .help("The iptables file to set"), // Set a description of this argument.
                ),
        )
        .get_matches(); // Parse the command line arguments and return a `clap::ArgMatches` struct.

    // Return an `Ok` value indicating success.
    Ok(())
}

//checking for subcommand entered form user 
    match matches.subcommand() {
        ("get", Some(get_matches)) => {
            // retrieve the sandbox ID from the command line arguments
            let sandbox_id = get_matches.value_of("sandbox-id").unwrap();
            // check if ipv6 is requested
            let is_ipv6 = get_matches.is_present("v6");
            // verify the container ID before proceeding
            verify_container_id(sandbox_id)?;
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
        ("set", Some(set_matches)) => {
            // Extract sandbox ID and IPv6 flag from command-line arguments
            let sandbox_id = set_matches.value_of("sandbox-id").unwrap();
            let is_ipv6 = set_matches.is_present("v6");
            let iptables_file = set_matches.value_of("file").unwrap();
            
            // Verify the specified sandbox ID is valid
            verify_container_id(sandbox_id)?;
        
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
