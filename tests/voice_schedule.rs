use meditate::audio::voice::{has_meditation_prompts, prompt_phase, VoicePhase, VoiceScheduler};
use meditate::pack::{MeditationPrompt, VoicePack};

fn prompt(id: &str, phase: &str) -> MeditationPrompt {
    MeditationPrompt {
        id: id.into(),
        seq: 0,
        duration_sec: 0.0,
        file_size_bytes: 0,
        r2_key: format!("voiceguide/gentle/{id}.aac"),
        phase: Some(phase.into()),
    }
}

#[test]
fn elapsed_time_maps_to_phase() {
    assert_eq!(prompt_phase(0), VoicePhase::Settling);
    assert_eq!(prompt_phase(179), VoicePhase::Settling);
    assert_eq!(prompt_phase(180), VoicePhase::Deepening);
    assert_eq!(prompt_phase(599), VoicePhase::Deepening);
    assert_eq!(prompt_phase(600), VoicePhase::Closing);
}

#[test]
fn scheduler_respects_delay_spacing_and_phase() {
    let mut scheduler = VoiceScheduler::new(vec![
        prompt("s1", "settling"),
        prompt("d1", "deepening"),
        prompt("c1", "closing"),
    ]);

    assert!(scheduler.next(20).is_none());
    assert_eq!(scheduler.next(30).map(|p| p.id), Some("s1".to_string()));
    assert!(scheduler.next(60).is_none());
    assert!(scheduler.next(120).is_none());
    assert_eq!(scheduler.next(200).map(|p| p.id), Some("d1".to_string()));
    assert_eq!(scheduler.next(620).map(|p| p.id), Some("c1".to_string()));
    assert!(scheduler.next(800).is_none());
}

#[test]
fn empty_pack_offers_nothing() {
    let mut scheduler = VoiceScheduler::new(vec![]);
    assert!(scheduler.is_empty());
    assert!(scheduler.next(120).is_none());
}

#[test]
fn meditation_prompts_gate_whether_a_pack_is_offered() {
    let walk_only = VoicePack {
        id: "guide".into(),
        name: "Guide".into(),
        tagline: "a calm guide".into(),
        meditation_prompts: vec![],
    };
    assert!(!has_meditation_prompts(&walk_only));

    let with_meditation = VoicePack {
        meditation_prompts: vec![prompt("m1", "settling")],
        ..walk_only
    };
    assert!(has_meditation_prompts(&with_meditation));
}
