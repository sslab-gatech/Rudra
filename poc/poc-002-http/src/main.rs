use http::header::HeaderMap;

fn main() {
    let mut map = HeaderMap::<u32>::with_capacity(8);
    map.insert("key1", 1);
    map.append("key1", 2);
    map.insert("key2", 3);
    map.append("key2", 4);

    let mut drain = map.drain();
    let (key1, mut val1) = drain.next().unwrap();
    let (key2, mut val2) = drain.next().unwrap();

    dbg!(val1.next());
    dbg!(val2.next());
    dbg!(val1.next());
    dbg!(val2.next());
}
