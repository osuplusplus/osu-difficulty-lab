//! 图表数据结构，对应上游 `js/patterns/chart.js`。
//!
//! 这里的字段命名与上游 JavaScript 保持一致，方便逐行核对移植结果。

/// 单个物件的类型，对应上游 `NoteType`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteType {
    Nothing,
    Normal,
    HoldHead,
    HoldBody,
    HoldTail,
}

/// 一行（同一时间的全部物件），对应上游 `createTimeItem` 的结果。
#[derive(Debug, Clone)]
pub struct Row {
    pub time: f64,
    pub data: Vec<NoteType>,
}

impl Row {
    pub fn new(time: f64, keys: usize) -> Self {
        Self {
            time,
            data: vec![NoteType::Nothing; keys],
        }
    }
}

/// 时间点，对应上游 `createTimeItem`。
#[derive(Debug, Clone, Copy)]
pub struct TimeItem<T> {
    pub time: f64,
    pub data: T,
}

/// 继承（BPM）时间点的数据，对应上游 `createBPM`。
#[derive(Debug, Clone, Copy)]
pub struct BpmData {
    pub meter: i64,
    pub ms_per_beat: f64,
}

/// 谱面，对应上游 `createChart`。
#[derive(Debug, Clone)]
pub struct Chart {
    pub keys: usize,
    pub notes: Vec<Row>,
    pub bpm: Vec<TimeItem<BpmData>>,
    pub sv: Vec<TimeItem<f64>>,
}

impl Chart {
    pub fn first_note(&self) -> f64 {
        self.notes[0].time
    }

    pub fn last_note(&self) -> f64 {
        self.notes[self.notes.len() - 1].time
    }

    pub fn duration(&self) -> f64 {
        self.last_note() - self.first_note()
    }
}
