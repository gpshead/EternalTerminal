/// Test SSH connection with password authentication
use et_terminal::ssh::{SshClient, SshConfig};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    println!("Testing SSH connection to localhost...");

    // Test 1: Password authentication
    let config_password = SshConfig {
        host: "localhost".to_string(),
        port: 22,
        user: "root".to_string(),
        identity_file: None,
        password: Some("testpassword123".to_string()),
    };

    match SshClient::connect(config_password) {
        Ok(client) => {
            println!("✓ SSH connection successful (password auth)");

            // Test command execution
            match client.execute_command("echo 'Hello from SSH'") {
                Ok(output) => {
                    println!("✓ Command execution successful");
                    println!("  Output: {}", output.trim());
                }
                Err(e) => {
                    println!("✗ Command execution failed: {}", e);
                }
            }
        }
        Err(e) => {
            println!("✗ SSH connection failed (password auth): {}", e);
        }
    }

    // Test 2: Key-based authentication
    println!("\nTesting key-based authentication...");
    let config_key = SshConfig {
        host: "localhost".to_string(),
        port: 22,
        user: "root".to_string(),
        identity_file: Some(PathBuf::from("/root/.ssh/id_ed25519")),
        password: None,
    };

    match SshClient::connect(config_key) {
        Ok(client) => {
            println!("✓ SSH connection successful (key auth)");

            match client.execute_command("whoami") {
                Ok(output) => {
                    println!("✓ Command execution successful");
                    println!("  User: {}", output.trim());
                }
                Err(e) => {
                    println!("✗ Command execution failed: {}", e);
                }
            }
        }
        Err(e) => {
            println!("✗ SSH connection failed (key auth): {}", e);
            println!("  (This is expected if key authentication isn't properly configured)");
        }
    }

    println!("\nSSH module test complete!");
    Ok(())
}
