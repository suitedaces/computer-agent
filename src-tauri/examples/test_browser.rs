// test browser tools
// run with: cargo run --example test_browser
// requires Chrome to be running with --remote-debugging-port=9222

use taskhomie_lib::browser::{BrowserClient, restart_chrome_with_debugging};

#[tokio::main]
async fn main() {
    println!("=== Browser Tools Test ===\n");

    // connect
    println!("Connecting to Chrome...");
    let mut client = match BrowserClient::connect().await {
        Ok(c) => {
            println!("✓ Connected\n");
            c
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("CHROME_NEEDS_RESTART") {
                println!("Chrome is running without debugging. Restarting...\n");
                match restart_chrome_with_debugging().await {
                    Ok(c) => {
                        println!("✓ Chrome restarted and connected\n");
                        c
                    }
                    Err(e) => {
                        println!("✗ Failed to restart Chrome: {}", e);
                        return;
                    }
                }
            } else {
                println!("✗ Failed to connect: {}", e);
                println!("\nMake sure Chrome is running. Try:");
                println!("  open -a 'Google Chrome' --args --remote-debugging-port=9222");
                return;
            }
        }
    };

    // list_pages
    println!("Testing list_pages...");
    match client.list_pages().await {
        Ok(pages) => println!("✓ {}\n", pages),
        Err(e) => println!("✗ {}\n", e),
    }

    // navigate
    println!("Testing navigate_page to example.com...");
    match client.navigate_page("url", Some("https://example.com"), false).await {
        Ok(r) => println!("✓ {}\n", r),
        Err(e) => println!("✗ {}\n", e),
    }

    // snapshot
    println!("Testing take_snapshot...");
    match client.take_snapshot(false).await {
        Ok(snap) => {
            let lines: Vec<&str> = snap.lines().collect();
            println!("✓ Got {} lines", lines.len());
            println!("First 10 lines:");
            for line in lines.iter().take(10) {
                println!("  {}", line);
            }
            println!();
        }
        Err(e) => println!("✗ {}\n", e),
    }

    // new_page
    println!("Testing new_page...");
    match client.new_page("https://httpbin.org/forms/post").await {
        Ok(r) => println!("✓ {}\n", r),
        Err(e) => println!("✗ {}\n", e),
    }

    // snapshot the form page
    println!("Testing take_snapshot on form page...");
    match client.take_snapshot(false).await {
        Ok(snap) => {
            let lines: Vec<&str> = snap.lines().collect();
            println!("✓ Got {} lines", lines.len());

            // find textboxes for fill_form test
            let textboxes: Vec<&str> = lines.iter()
                .filter(|l| l.contains("textbox"))
                .copied()
                .collect();
            println!("Found {} textboxes:", textboxes.len());
            for tb in textboxes.iter().take(3) {
                println!("  {}", tb);
            }
            println!();
        }
        Err(e) => println!("✗ {}\n", e),
    }

    // select_page back to first
    println!("Testing select_page(0)...");
    match client.select_page(0, true).await {
        Ok(r) => println!("✓ {}\n", r),
        Err(e) => println!("✗ {}\n", e),
    }

    // list pages again
    println!("Testing list_pages (should show 2 tabs)...");
    match client.list_pages().await {
        Ok(pages) => println!("✓ {}\n", pages),
        Err(e) => println!("✗ {}\n", e),
    }

    // close_page
    println!("Testing close_page(1)...");
    match client.close_page(1).await {
        Ok(r) => println!("✓ {}\n", r),
        Err(e) => println!("✗ {}\n", e),
    }

    // final list
    println!("Final list_pages...");
    match client.list_pages().await {
        Ok(pages) => println!("✓ {}\n", pages),
        Err(e) => println!("✗ {}\n", e),
    }

    println!("=== Test Complete ===");
}
