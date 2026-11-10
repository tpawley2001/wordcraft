use super::{grpprl, one_section, read, struct_doc};
#[test]
fn dbg_sepx() {
    let prls: Vec<(u16, &[u8])> = vec![(0x702B, &[0x0C, 0x01]), (0xD234, &[7, 0, 0, 0, 0, 0x10, 0x03, 0x04, 0])];
    let spec = one_section("Text\r", grpprl(&prls), vec![(0, 0, Vec::new())]);
    let bytes = struct_doc(&spec);
    let pos = bytes.windows(2).position(|w| w == [0x2B, 0x70]).unwrap();
    let mut word = bytes.clone();
    word[pos + 2] = 0x0C;
    word[pos + 3] = 0x01;
    word[pos + 4] = 0x00;
    word[pos + 5] = 0x00;
    let mut comp = crate::open_container(&word).unwrap();
    let w = crate::stream(&mut comp, "WordDocument").unwrap();
    let p = w.windows(2).position(|w| w == [0x2B, 0x70]).unwrap();
    let g = &w[p - 2..p + 15];
    eprintln!("{:?}", g);
    let mut it = crate::sprm::iter(g);
    for _ in 0..5 { eprintln!("{:?}", it.next().map(|p| (p.op, p.operand.to_vec()))); }
    let doc = read(&word).expect("opens");
    eprintln!("patched {:?}", doc.last_section.page_borders);
}
