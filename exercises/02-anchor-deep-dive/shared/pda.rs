// Given helper for the capstone crates: the same code as exercises 2.2 in
// `solutions`, so the capstone does not depend on your starter work.

/// `Pubkey::create_program_address`: sha256(seeds.. || [bump] ||
/// program_id || "ProgramDerivedAddress"); reject on-curve results
/// (`InvalidSeeds`) and seeds over the limits (`MaxSeedLengthExceeded`).
pub fn create_program_address(
    seeds: &[&[u8]],
    bump: u8,
    program_id: &Pubkey,
) -> Result<Pubkey, AnchorError> {
    if seeds.len() >= MAX_SEEDS || seeds.iter().any(|s| s.len() > MAX_SEED_LEN) {
        return Err(AnchorError::MaxSeedLengthExceeded);
    }
    let bump = [bump];
    let mut parts: Vec<&[u8]> = seeds.to_vec();
    parts.push(&bump);
    parts.push(&program_id.0);
    parts.push(PDA_MARKER);
    let h = sha256(&parts);
    if is_on_curve(&h) {
        return Err(AnchorError::InvalidSeeds);
    }
    Ok(Pubkey(h))
}

/// `Pubkey::find_program_address`: try bumps 255, 254, ... 0 and
/// return the first off-curve address with its (canonical) bump.
pub fn find_program_address(
    seeds: &[&[u8]],
    program_id: &Pubkey,
) -> Result<(Pubkey, u8), AnchorError> {
    for bump in (0..=u8::MAX).rev() {
        match create_program_address(seeds, bump, program_id) {
            Ok(k) => return Ok((k, bump)),
            Err(AnchorError::InvalidSeeds) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(AnchorError::InvalidSeeds)
}
