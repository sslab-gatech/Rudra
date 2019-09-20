mod inner {
    pub static MSG: &str = "YES";
}

fn main() {
    println!("Hello, World!");
    println!("{}", inner::MSG);
}
