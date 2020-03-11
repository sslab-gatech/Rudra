use http::header::HeaderMap;

struct DropDetector(u32);

impl Drop for DropDetector {
    fn drop(&mut self) {
        println!("Dropping {}", self.0);
    }
}

fn main() {
    {
        println!("Failing to drop Drain causes double-free");

        let mut map = HeaderMap::with_capacity(32);
        map.insert("1", DropDetector(1));
        map.insert("2", DropDetector(2));

        let mut drain = map.drain();
        drain.next();
        std::mem::forget(drain);
    }

    {
        println!("Drop drain without consuming it leaks memory");

        let mut map = HeaderMap::with_capacity(32);
        map.insert("3", DropDetector(3));
        map.insert("4", DropDetector(4));

        let mut drain = map.drain();
        drain.next();
    }
}
