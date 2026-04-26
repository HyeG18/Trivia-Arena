fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Le dice a Cargo que compile nuestros archivos proto
    tonic_build::compile_protos("../../proto/game.proto")?;
    Ok(())
}