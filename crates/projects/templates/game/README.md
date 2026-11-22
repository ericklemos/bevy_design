# Seu jogo Bevy

Este projeto não depende do editor nem do repositório que o gerou. Use Rust
1.97.1 ou mais recente. No Debian/Ubuntu, instale `pkg-config`, `libwayland-dev`
e `libxkbcommon-dev` para compilar o suporte a janelas.

Na raiz do jogo:

```sh
cargo run
cargo build --release
```

O jogo abre uma janela com a cena inicial indicada em `project.ron`. Projetos
novos começam com uma cena vazia. Edite `src/main.rs` para adicionar a lógica
do jogo e alterar a câmera inicial.

Para distribuir, copie `target/release/bevy_game` (`bevy_game.exe` no Windows)
para uma pasta junto com `project.ron`, `scenes/` e `assets/`. Compile para a
plataforma de destino. Quem recebe o pacote não precisa do editor nem do Cargo.
