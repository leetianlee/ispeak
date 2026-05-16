//! Export a Transcript in various formats. 3.1 ships Markdown + plain text.

use crate::meeting::types::{ExportFormat, SpeakerLabel, Transcript, TranscriptSource};

pub fn render(transcript: &Transcript, format: ExportFormat) -> String {
    match format {
        ExportFormat::Markdown => render_markdown(transcript),
        ExportFormat::PlainText => render_plain(transcript),
        _ => render_plain(transcript),  // JSON/SRT/VTT come in slice 3.3
    }
}

fn render_markdown(t: &Transcript) -> String {
    let mut s = String::new();
    s.push_str("# Transcript\n\n");
    s.push_str(&format!("- **Date**: {}\n", format_timestamp(t.created_at)));
    s.push_str(&format!("- **Duration**: {}\n", format_duration(t.duration_secs)));
    s.push_str(&format!("- **Source**: {}\n", format_source(&t.source)));
    if t.partial {
        s.push_str("- **Status**: ⚠ Partial (some chunks failed or job was cancelled)\n");
    }
    s.push_str("\n## Transcript\n\n");
    for seg in &t.segments {
        let speaker = label(&seg.speaker);
        s.push_str(&format!("**{}**: {}\n\n", speaker, seg.text));
    }
    s
}

fn render_plain(t: &Transcript) -> String {
    let mut s = String::new();
    for seg in &t.segments {
        s.push_str(&format!("{}: {}\n", label(&seg.speaker), seg.text));
    }
    s
}

fn label(l: &SpeakerLabel) -> String {
    match l {
        SpeakerLabel::You => "You".into(),
        SpeakerLabel::Other => "Speaker".into(),
        SpeakerLabel::Indexed(n) => format!("Speaker {}", (b'A' + *n) as char),
    }
}

fn format_duration(secs: f32) -> String {
    let mins = (secs / 60.0).floor() as u32;
    let s = (secs - (mins as f32 * 60.0)).round() as u32;
    format!("{mins}m {s}s")
}

fn format_timestamp(millis: u64) -> String {
    // ISO-like, no chrono dep; YYYY-MM-DDTHH:MM:SSZ in UTC.
    let secs = (millis / 1000) as i64;
    let (d, t) = (secs / 86_400, secs % 86_400);
    let mut y = 1970_i64;
    let mut days = d;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let y_days = if leap { 366 } else { 365 };
        if days < y_days { break; }
        days -= y_days;
        y += 1;
    }
    let mdays = [31,28,31,30,31,30,31,31,30,31,30,31];
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let mut m = 0usize;
    let mut day = days;
    loop {
        let dm = mdays[m] + if m == 1 && leap { 1 } else { 0 };
        if day < dm { break; }
        day -= dm;
        m += 1;
    }
    let hh = t / 3600;
    let mm = (t % 3600) / 60;
    let ss = t % 60;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m + 1, day + 1, hh, mm, ss)
}

fn format_source(s: &TranscriptSource) -> String {
    match s {
        TranscriptSource::FileImport(p) => format!("file: {}", p.display()),
        TranscriptSource::LiveCapture => "live capture".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::types::Segment;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn sample_transcript() -> Transcript {
        Transcript {
            id: Uuid::nil(),
            created_at: 1_715_000_000_000,
            duration_secs: 73.0,
            source: TranscriptSource::FileImport(PathBuf::from("/tmp/meeting.m4a")),
            segments: vec![
                Segment { start: 0.0, end: 2.0, speaker: SpeakerLabel::Other, text: "Hello team.".into() },
                Segment { start: 2.0, end: 5.0, speaker: SpeakerLabel::Indexed(1), text: "Hi.".into() },
            ],
            summary: None,
            action_items: vec![],
            partial: false,
        }
    }

    #[test]
    fn markdown_contains_segments_and_metadata() {
        let md = render(&sample_transcript(), ExportFormat::Markdown);
        assert!(md.starts_with("# Transcript"));
        assert!(md.contains("- **Duration**: 1m 13s"));
        assert!(md.contains("**Speaker**: Hello team."));
        assert!(md.contains("**Speaker B**: Hi."));
    }

    #[test]
    fn markdown_flags_partial_transcripts() {
        let mut t = sample_transcript();
        t.partial = true;
        let md = render(&t, ExportFormat::Markdown);
        assert!(md.contains("⚠ Partial"));
    }

    #[test]
    fn plain_text_has_one_line_per_segment() {
        let txt = render(&sample_transcript(), ExportFormat::PlainText);
        let lines: Vec<&str> = txt.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);
    }
}
