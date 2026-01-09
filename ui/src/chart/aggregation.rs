use core::Candle;

#[derive(Clone, Copy, Debug)]
pub(crate) struct AggregatedCandle {
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
}

impl AggregatedCandle {
    pub(crate) fn from_slice(candles: &[Candle]) -> Option<Self> {
        let first = candles.first()?;
        let last = candles.last()?;
        let mut high = first.high;
        let mut low = first.low;
        let mut volume = 0.0_f64;
        for c in candles {
            high = high.max(c.high);
            low = low.min(c.low);
            volume += c.volume;
        }
        Some(Self {
            open: first.open,
            close: last.close,
            high,
            low,
            volume,
        })
    }
}
