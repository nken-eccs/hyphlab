use crate::GraphemeIndex;
use smallvec::SmallVec;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone)]
pub struct BoundaryMap<'a> {
    pub word: &'a str,
    pub grapheme_to_byte: SmallVec<[usize; 32]>,
    pub byte_to_grapheme: SmallVec<[(usize, GraphemeIndex); 32]>,
    pub char_to_byte: SmallVec<[usize; 32]>,
}

impl<'a> BoundaryMap<'a> {
    pub fn new(word: &'a str) -> Self {
        let mut grapheme_to_byte = SmallVec::new();
        let mut byte_to_grapheme = SmallVec::new();

        for (idx, (byte, _)) in word.grapheme_indices(true).enumerate() {
            grapheme_to_byte.push(byte);
            byte_to_grapheme.push((byte, idx as GraphemeIndex));
        }

        let end = word.len();
        let end_idx = grapheme_to_byte.len() as GraphemeIndex;
        grapheme_to_byte.push(end);
        byte_to_grapheme.push((end, end_idx));

        let mut char_to_byte = SmallVec::new();
        for (byte, _) in word.char_indices() {
            char_to_byte.push(byte);
        }
        char_to_byte.push(end);

        Self {
            word,
            grapheme_to_byte,
            byte_to_grapheme,
            char_to_byte,
        }
    }

    pub fn grapheme_len(&self) -> usize {
        self.grapheme_to_byte.len().saturating_sub(1)
    }

    pub fn byte_to_grapheme_break(&self, byte: usize) -> Option<GraphemeIndex> {
        self.byte_to_grapheme
            .iter()
            .find_map(|(b, idx)| (*b == byte).then_some(*idx))
    }

    pub fn grapheme_to_byte_break(&self, index: GraphemeIndex) -> Option<usize> {
        self.grapheme_to_byte.get(index as usize).copied()
    }

    pub fn segment_lengths_to_breaks<I>(
        &self,
        segments: I,
        out: &mut SmallVec<[GraphemeIndex; 8]>,
    ) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = usize>,
    {
        out.clear();
        let mut byte = 0usize;
        let word_len = self.word.len();

        for len in segments {
            byte = byte.saturating_add(len);
            if byte >= word_len {
                continue;
            }
            let Some(grapheme) = self.byte_to_grapheme_break(byte) else {
                anyhow::bail!("segment ended inside a grapheme at byte {byte}");
            };
            out.push(grapheme);
        }

        Ok(())
    }
}
