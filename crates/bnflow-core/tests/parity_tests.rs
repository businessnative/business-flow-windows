use bnflow_core::*;
use chrono::TimeZone;

fn iso(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(s).unwrap().timestamp()
}

#[test]
fn history_stats_and_prune_match_mac() {
    let now = 1_700_000_000_000i64;
    let items = vec![
        HistoryItem { id: uuid::Uuid::nil(), date_ms: now, text: "egy kettő három".into(), raw_text: "egy kettő három".into(), app_name: "A".into(), duration_seconds: 3.0, kind: HistoryKind::Dictation },
        HistoryItem { id: uuid::Uuid::nil(), date_ms: now - 86_400_000, text: "négy öt".into(), raw_text: "négy öt".into(), app_name: "A".into(), duration_seconds: 2.0, kind: HistoryKind::Dictation },
        HistoryItem { id: uuid::Uuid::nil(), date_ms: now - 40 * 86_400_000, text: "régi".into(), raw_text: "régi".into(), app_name: "A".into(), duration_seconds: 0.0, kind: HistoryKind::Dictation },
    ];
    let st = HistoryLogic::stats(&items, now);
    assert_eq!(st.total_words, 6);
    assert_eq!(st.week_words, 5);
    assert_eq!(st.streak_days, 2);
    assert_eq!(st.average_wpm, 60);
    assert_eq!(HistoryLogic::prune(&items, HistoryRetention::Days30, now).len(), 2);
    assert!(HistoryLogic::prune(&items, HistoryRetention::Never, now).is_empty());
}

#[test]
fn history_search_ignores_accents() {
    let items = vec![HistoryItem::new("Mi a kérdés?".into(), "kerdes".into(), "Mail".into(), 1.2)];
    assert_eq!(HistorySearch::filter(&items, "kerdes").len(), 1);
    assert!(HistorySearch::filter(&items, "nincs").is_empty());
}

#[test]
fn word_diff_and_dictionary_edit() {
    assert_eq!(
        WordDiff::diff("a b c", "a x c"),
        vec![DiffOp::Same("a".into()), DiffOp::Removed("b".into()), DiffOp::Added("x".into()), DiffOp::Same("c".into())]
    );
    let e = DictionaryEditing::make("ClickUp", "klikkap, click up, ClickUp").unwrap();
    assert_eq!(e.heard_as, vec!["klikkap".to_string(), "click up".to_string()]);
    let list = DictionaryEditing::insert(e, &[]);
    assert_eq!(list[0].term, "ClickUp");
}

#[test]
fn autolearn_matches_mac_fixture() {
    let inserted = "Köszönöm, Szőts úr.";
    let c = AutoLearn::correction(inserted, "Kedves Úr! Köszönöm, Szőts úr.", "Kedves Úr! Köszönöm, Szőcs úr.", &[]);
    assert_eq!(c.as_ref().map(|x| x.0.as_str()), Some("Szőts"));
    assert_eq!(c.as_ref().map(|x| x.1.as_str()), Some("Szőcs"));
    assert!(AutoLearn::correction(inserted, "Köszönöm, Szőts úr.", "Teljesen más szöveg itt.", &[]).is_none());
    assert!(AutoLearn::correction(inserted, "Köszönöm, Szőts úr.", "Köszönöm, Kovács úr.", &[]).is_none());
    let mut d = vec![];
    let (heard, term) = c.unwrap();
    AutoLearn::learn(&heard, &term, &mut d);
    assert_eq!(d[0].term, "Szőcs");
    assert!(d[0].auto_learned);
}

#[test]
fn latency_policy_defaults_off_and_budgets() {
    assert!(!AppSettings::default().paid_prefetch);
    let off = CallBudget::worst_case(false);
    let on = CallBudget::worst_case(true);
    assert_eq!(off, (1, 1));
    assert_eq!(on, (1 + SpeculationPolicy::MAX_STARTS, 1 + SpeculationPolicy::MAX_STARTS));
    assert_eq!(CallBudget::per_take(false, 2).0, 1);
    assert_eq!(CallBudget::per_take(true, 2).0, 3);
    assert_eq!(CallBudget::per_take(true, 99).1, 3);
    assert!(PasteConfirm::should_poll(0, false, true));
    assert!(!PasteConfirm::should_poll(0, true, true));
    assert!(!PasteConfirm::should_poll(PasteConfirm::MAX_POLLS, false, true));
    let trace = DictationTrace { finalize_ms: 1, transcribe_wait_ms: 2, overlapped_ms: 900, cleanup_ms: 3, paste_ms: 4, post_speech_ms: 5, speculated: true, bytes: 6 };
    let line = trace.log_line();
    assert!(line.contains("overlapped=900ms") && line.contains("spec=1"));
    assert!(!line.contains("sk-"));
}

#[test]
fn meeting_ics_matches_mac_fixture() {
    let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:one\r\nDTSTART;TZID=Europe/Budapest:20261104T100000\r\nDTEND;TZID=Europe/Budapest:20261104T110000\r\nSUMMARY:Heti egyeztetés\\, csapat\r\nDESCRIPTION:Csatlakozás: https://meet.google.com/abc-de\r\n fg-hij\\nKöszi\r\nBEGIN:VALARM\r\nDESCRIPTION:riasztás\r\nEND:VALARM\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:rec\r\nDTSTART:20261102T080000Z\r\nDTEND:20261102T083000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE;COUNT=4\r\nEXDATE:20261104T080000Z\r\nLOCATION:https://meet.google.com/xyz-wxyz-xyz\r\nSUMMARY:Standup\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:rec\r\nRECURRENCE-ID:20261109T080000Z\r\nDTSTART:20261109T090000Z\r\nDTEND:20261109T093000Z\r\nLOCATION:https://meet.google.com/xyz-wxyz-xyz\r\nSUMMARY:Standup (áthelyezve)\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:nolink\r\nDTSTART:20261104T120000Z\r\nSUMMARY:Ebéd\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:allday\r\nDTSTART;VALUE=DATE:20261105\r\nDESCRIPTION:https://meet.google.com/aaa-bbbb-ccc\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:cancel\r\nSTATUS:CANCELLED\r\nDTSTART:20261104T130000Z\r\nDESCRIPTION:https://meet.google.com/aaa-bbbb-ccc\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    assert_eq!(MeetingLinks::find("Join: https://meet.google.com/abc-defg-hij ok").unwrap().1, MeetingPlatform::GoogleMeet);
    assert!(MeetingLinks::find("https://meet.google.com/landing").is_none());
    let ev = Ics::meetings(ics, iso("2026-11-01T00:00:00Z"), iso("2026-11-20T00:00:00Z"), chrono_tz::UTC);
    let one = ev.iter().find(|e| e.uid == "one").unwrap();
    assert_eq!(one.title, "Heti egyeztetés, csapat");
    assert_eq!(one.start, iso("2026-11-04T09:00:00Z"));
    assert_eq!(one.url, "https://meet.google.com/abc-defg-hij");
    let stand: Vec<_> = ev.iter().filter(|e| e.uid == "rec").map(|e| e.start).collect();
    assert_eq!(stand, vec![iso("2026-11-02T08:00:00Z"), iso("2026-11-09T09:00:00Z"), iso("2026-11-11T08:00:00Z")]);
    assert!(ev.iter().all(|e| !["nolink", "allday", "cancel"].contains(&e.uid.as_str())));
    let window = Ics::meetings(ics, iso("2026-11-10T00:00:00Z"), iso("2026-11-12T00:00:00Z"), chrono_tz::UTC);
    assert_eq!(window.iter().map(|e| e.uid.as_str()).collect::<Vec<_>>(), vec!["rec"]);
    let slug_event = MeetingEvent { uid: "u".into(), title: "A/B: teszt?".into(), start: iso("2026-11-04T09:00:00Z"), end: iso("2026-11-04T10:00:00Z"), url: "https://meet.google.com/abc-defg-hij".into(), platform: MeetingPlatform::GoogleMeet };
    assert_eq!(MeetingNotes::slug(&slug_event, chrono_tz::UTC), "2026-11-04 0900 – A B  teszt");
}

#[test]
fn meeting_chunks_captions_and_summary_limit() {
    let pcm = vec![0u8; 16_000 * 2 * 25];
    let c = MeetingAudio::chunks(&pcm, 10);
    assert_eq!(c.iter().map(|x| x.len()).collect::<Vec<_>>(), vec![320_000, 320_000, 160_000]);
    assert!(MeetingAudio::is_silent(&c[0]));
    assert_eq!(MeetingAudio::wav_len(c[2].len()), 160_044);
    let s = MeetingNotes::summary_input("T", &"a".repeat(300), 90);
    assert!(s.contains("kimaradt") && s.chars().count() < 250);
    let j = "{\"speaker\":\"Anna\",\"text\":\"Szia, kezdjük\",\"t\":1,\"tEnd\":3}\n{\"speaker\":\"Anna\",\"text\":\"Szia, kezdjük a költségvetéssel\",\"t\":2,\"tEnd\":6}\n{\"speaker\":\"Béla\",\"text\":\"Rendben, péntekig küldöm\",\"t\":8,\"tEnd\":20}\nrossz sor\n{\"speaker\":\"\",\"text\":\"x\",\"t\":9}\n";
    let caps = MeetingCaptions::parse(j);
    assert_eq!(caps.len(), 2);
    assert_eq!(caps[0].text, "Szia, kezdjük a költségvetéssel");
    assert_eq!(MeetingCaptions::talk_time(&caps)[0].0, "Béla");
    assert!(MeetingCaptions::markdown(&caps).contains("**Béla** [00:00:08]: Rendben"));
}

#[test]
fn quota_message_is_explicit() {
    assert_eq!(FlowError::Quota.message(), "Elfogyott az átírás kerete. Nézd meg az OpenAI vagy a Gemini számlázását.");
}

#[test]
fn onboarding_counts_steps() {
    let st = OnboardingState { has_key: true, microphone: true, accessibility: false, input_monitoring: false, has_dictated: false };
    assert_eq!(st.done_count(), 2);
    assert_eq!(st.next(), Some(2));
    assert!(!st.complete());
}
