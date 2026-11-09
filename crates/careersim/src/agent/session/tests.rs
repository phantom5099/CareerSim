use super::SessionId;

#[test]
fn session_id_parses_its_display_form() {
    let id = SessionId::generate();
    assert_eq!(id.to_string().parse::<SessionId>().unwrap(), id);
}

#[test]
fn session_id_rejects_invalid_text() {
    assert!("not-a-uuid".parse::<SessionId>().is_err());
    assert!("".parse::<SessionId>().is_err());
}
