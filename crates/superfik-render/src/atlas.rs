//! Shelf packing for texture atlases. Pure bookkeeping, no GPU calls.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasSlot {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy)]
struct Shelf {
    y: u32,
    height: u32,
    cursor: u32,
}

#[derive(Debug, Clone)]
pub struct ShelfPacker {
    size: u32,
    padding: u32,
    shelves: Vec<Shelf>,
}

impl ShelfPacker {
    pub fn new(size: u32, padding: u32) -> Self {
        Self { size, padding, shelves: Vec::new() }
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn allocate(&mut self, w: u32, h: u32) -> Option<AtlasSlot> {
        let (pw, ph) = (w + self.padding * 2, h + self.padding * 2);
        if pw > self.size || ph > self.size {
            return None;
        }
        let fits = |s: &Shelf| s.height >= ph && s.height <= ph + ph / 2 && s.cursor + pw <= self.size;
        let index = match self.shelves.iter().position(fits) {
            Some(index) => index,
            None => {
                let y = self.shelves.last().map_or(0, |s| s.y + s.height);
                if y + ph > self.size {
                    return None;
                }
                self.shelves.push(Shelf { y, height: ph, cursor: 0 });
                self.shelves.len() - 1
            }
        };
        let shelf = &mut self.shelves[index];
        let slot = AtlasSlot { x: shelf.cursor + self.padding, y: shelf.y + self.padding, w, h };
        shelf.cursor += pw;
        Some(slot)
    }

    pub fn clear(&mut self) {
        self.shelves.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_in_rows_with_padding() {
        let mut packer = ShelfPacker::new(64, 1);
        let a = packer.allocate(10, 10).expect("a");
        let b = packer.allocate(10, 10).expect("b");
        assert_eq!((a.x, a.y), (1, 1));
        assert_eq!((b.x, b.y), (13, 1));
    }

    #[test]
    fn opens_new_shelf_when_row_is_full() {
        let mut packer = ShelfPacker::new(32, 0);
        packer.allocate(20, 8).expect("first");
        let second = packer.allocate(20, 8).expect("second");
        assert_eq!((second.x, second.y), (0, 8));
    }

    #[test]
    fn rejects_when_full_or_too_big() {
        let mut packer = ShelfPacker::new(16, 0);
        assert!(packer.allocate(17, 1).is_none());
        packer.allocate(16, 16).expect("fills");
        assert!(packer.allocate(1, 1).is_none());
        packer.clear();
        assert!(packer.allocate(1, 1).is_some());
    }

    #[test]
    fn very_different_heights_use_separate_shelves() {
        let mut packer = ShelfPacker::new(64, 0);
        packer.allocate(4, 30).expect("tall");
        let short = packer.allocate(4, 4).expect("short");
        assert_eq!(short.y, 30);
    }
}
