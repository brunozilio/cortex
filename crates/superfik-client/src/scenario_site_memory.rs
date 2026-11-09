//! What the agent remembers in the website's story (`scenario_site`): a
//! small business's facts, habits and recent deals, in English or Portuguese.

use superfik_core::{Author, Memory, MemoryId, MemoryKind};

pub(super) fn memories(english: bool) -> Vec<Memory> {
    use MemoryKind::{Episodic as E, Procedural as P, Semantic as S};
    let pick = |en: &str, pt: &str| if english { en.to_owned() } else { pt.to_owned() };
    let entries: [(MemoryKind, &str, &str, &[&str]); 12] = [
        (S, "Acme prefers quarterly billing.", "A Acme prefere pagamento trimestral.", &["acme", "billing"]),
        (S, "Ana is Acme's head of operations.", "A Ana é a diretora de operações da Acme.", &["acme"]),
        (P, "Proposals go out as a PDF with the logo.", "Propostas saem em PDF com o logo.", &["proposal"]),
        (E, "Closed the Acme pilot for $18,000.", "Fechamos o piloto da Acme por R$ 18.000.", &["acme", "deal"]),
        (S, "Discounts above 10% need a yes.", "Descontos acima de 10% precisam de ok.", &["billing"]),
        (P, "Follow up two days after a proposal.", "Follow-up dois dias depois da proposta.", &["proposal"]),
        (S, "Invoices are due on the 10th.", "As faturas vencem no dia 10.", &["billing"]),
        (E, "Northwind signed the yearly plan.", "A Northwind assinou o plano anual.", &["deal"]),
        (S, "Write to clients in a warm, short tone.", "Escrever para clientes num tom curto.", &["tone"]),
        (E, "Sent the Globex renewal.", "Enviamos a renovação da Globex.", &["deal", "billing"]),
        (P, "Keep pricing.xlsx up to date.", "Manter precos.xlsx atualizada.", &["billing"]),
        (S, "Acme wants three teams this year.", "A Acme quer chegar a três times este ano.", &["acme"]),
    ];
    entries
        .iter()
        .enumerate()
        .map(|(i, (kind, en, pt, tags))| Memory {
            id: MemoryId(i as u32),
            kind: *kind,
            text: pick(en, pt),
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            created: 1_780_000_000 + i as i64 * 3_600,
            origin: Some(pick("Earlier sessions", "Sessões anteriores")),
            author: Author::Agent,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_hold_the_same_memories() {
        let (en, pt) = (memories(true), memories(false));
        assert_eq!(en.len(), pt.len());
        assert!(en[0].text.contains("quarterly") && pt[0].text.contains("trimestral"));
    }
}
