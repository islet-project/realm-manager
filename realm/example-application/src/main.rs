use std::time::Duration;

fn main() {
    let mut i = 0usize;
    loop {
        println!("I'm alive {}", i);

        i += 1;

        std::thread::sleep(Duration::from_secs(1));
    }
}
