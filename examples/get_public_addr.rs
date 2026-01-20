use pingo::get_public_addr;

fn main() {
    match get_public_addr() {
        Ok(addr) => println!("Your public address: {}", addr),
        Err(e) => eprintln!("Error: {}", e),
    }
}
