//! Which field of a page takes the username and which the password, as
//! Bitwarden's `generateLoginFillScript` chooses them.

use super::fields::PageField;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FillPlan {
    pub username: Option<u32>,
    pub password: Option<u32>,
}

pub fn plan(fields: &[PageField]) -> Option<FillPlan> {
    let (passwords, others) = passwords(fields);
    let without_forms = fields.iter().all(|f| f.form.is_none());
    let focused = fields.iter().position(|f| f.focused);

    let password = match focused {
        Some(at) if fields[at].is_password() => Some(at),
        Some(at) if is_username_candidate(&fields[at]) => {
            let beside = passwords.iter().chain(&others).copied().find(|&p| match fields[at].form {
                Some(form) => fields[p].form == Some(form),
                None => username_for(fields, p, without_forms) == Some(at),
            });
            let Some(password) = beside else {
                return Some(FillPlan { username: Some(fields[at].reference), password: None });
            };
            return Some(FillPlan { username: Some(fields[at].reference), password: Some(fields[password].reference) });
        }
        _ => passwords.first().copied(),
    };

    let Some(password) = password else {
        let username = fields.iter().find(|f| is_username_candidate(f) && f.names_username())?;
        return Some(FillPlan { username: Some(username.reference), password: None });
    };
    Some(FillPlan {
        username: username_for(fields, password, without_forms).map(|at| fields[at].reference),
        password: Some(fields[password].reference),
    })
}

fn passwords(fields: &[PageField]) -> (Vec<usize>, Vec<usize>) {
    let all: Vec<usize> = (0..fields.len()).filter(|&i| fields[i].is_password()).collect();
    let registers = |i: usize| {
        fields[i].wants_new_password()
            || fields[i].form.is_some_and(|form| all.iter().filter(|&&j| fields[j].form == Some(form)).count() > 1)
    };
    let (sign_in, register): (Vec<usize>, Vec<usize>) = all.iter().partition(|&&i| !registers(i));
    if sign_in.is_empty() { (register, sign_in) } else { (sign_in, register) }
}

fn is_username_candidate(field: &PageField) -> bool {
    field.takes_text() && field.usable() && !field.names_something_else()
}

fn username_for(fields: &[PageField], password: usize, without_forms: bool) -> Option<usize> {
    let form = fields[password].form;
    let before = || (0..password).rev().filter(|&i| is_username_candidate(&fields[i]));
    before()
        .find(|&i| form.is_some() && fields[i].form == form)
        .or_else(|| before().find(|&i| without_forms || fields[i].names_username()))
}
