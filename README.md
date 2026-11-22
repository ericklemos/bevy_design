# Engine Visual Nativa para Bevy

Editor nativo em **Bevy 0.20.1**.

## Configuração do projeto

Use **Rust 1.97.1** ou mais recente. No Debian/Ubuntu, instale as dependências
de compilação do Wayland antes de executar o editor pela primeira vez:

```sh
sudo apt install pkg-config libwayland-dev libxkbcommon-dev
```

Em seguida, na raiz do projeto:

```sh
cargo run -p editor
```

No Linux, o editor prefere **Wayland nativo** quando a sessão o disponibiliza,
sem depender do XWayland. Sessões X11 continuam suportadas.

Se a compilação indicar `Package wayland-client was not found`, confira se
`libwayland-dev` está instalado: ele fornece o arquivo `wayland-client.pc`.
Com a instalação pelo `apt`, não é necessário configurar `PKG_CONFIG_PATH`.

## Projetos

Na tela **Projetos**, use **Novo projeto**, informe o nome e clique em
**Procurar** para escolher onde criar a pasta do projeto. O editor preenche
o caminho com uma subpasta de mesmo nome. Você também pode digitar o caminho
completo de uma pasta nova. **Abrir projeto** aceita uma pasta com
`project.ron`. No editor, o botão **Procurar** abre o mesmo formulário; o próximo
projeto abre em outra janela sem fechar a atual.

A tela inicial mostra os últimos 20 projetos abertos, com busca por nome ou
caminho. Clique no projeto para reabri-lo ou em **×** para removê-lo do histórico
sem apagar arquivos. O histórico é local ao usuário: no Linux fica em
`$XDG_DATA_HOME/bevy-editor/user.redb` (por padrão, `~/.local/share/bevy-editor/user.redb`).

Também é possível criar e abrir pelo terminal:

```sh
cargo run -p editor -- new ../meu-jogo "Meu jogo"
cargo run -p editor -- open ../meu-jogo
cargo run -p editor -- open examples/demo
```

Cada projeto novo é um jogo Bevy independente: tem `Cargo.toml`, `src/main.rs`,
`assets/`, uma cena vazia em `scenes/main.scn.ron` e `project.ron`. O manifesto
guarda a versão, o nome e `startup_scene` (caminho relativo a `scenes/`). O layout
do grafo fica em `.editor/` de cada projeto.

Na pasta do jogo, use `cargo run` para abrir a janela do jogo e
`cargo build --release` para compilar o executável. Para distribuir, coloque
`target/release/bevy_game` (`bevy_game.exe` no Windows) junto com `project.ron`,
`scenes/` e `assets/`. O jogo não depende do editor nem do workspace que o gerou.
Projetos antigos continuam abrindo no editor, mas não ganham código Rust
automaticamente.

As alterações nas entidades feitas no editor ainda não são gravadas na cena.
Comandos sem caminho usam a pasta atual se ela tiver um manifesto;
`BEVY_ASSET_ROOT` também pode apontar para a pasta do projeto.

## Editor

Os painéis podem ser movidos, redimensionados e reacoplados pelas abas.
Em `Editors → Restaurar layout inicial` é possível voltar à disposição padrão.
O topo é dividido em três zonas, como no Blender: à esquerda ficam os menus
normais (`File`, `Procurar`, `Editors`, `Help`); no centro, os modos
**Building** e **Scripting**; à direita, o estado atual de edição ou simulação.
Building é o espaço de construção da cena; Scripting é o espaço de lógica, e o
Flow Graph só pode ser aberto nele. As barras laterais agrupam atalhos de
ferramentas no topo e no rodapé. Clique num ícone para abrir a ferramenta ou
ativar a sua aba; clique de novo para ocultá-la. A barra de status inferior está
reservada para informações futuras. O tema Warm Charcoal usa cards flutuantes:
os painéis são ilhas arredondadas separadas por respiros, com sombra leve em vez
de linhas divisórias. As abas ativas aparecem como pílulas.

O layout de Building tem três colunas: Hierarchy e Asset Browser à esquerda,
Viewport sobre Console no centro, e Inspector à direita. Ao entrar em Scripting,
os ícones de Hierarchy e Asset Browser dão lugar a **Schedule**, **Queries** e
**Observers**, que escolhem a vista do grafo. O Flow Graph ocupa o lugar da
Viewport, o Inspector passa a mostrar os detalhes do nó selecionado e o Console
é minimizado em uma faixa na parte inferior.
O pacote oficial de ícones do editor é o **Material Symbols Outlined**, do Google.
A fonte é embutida no binário e funciona offline. Para adicionar um ícone, siga
as [instruções do pacote](crates/editor/assets/material-symbols/README.md).
Na Viewport, arraste com o botão direito para olhar, use WASD/QE durante o arrasto
para mover e a roda para aproximar ou afastar. Na demo, o cubo é uma prévia 3D da entidade
`Player`: segue seu `Transform` e responde, pela cor, à proporção de vida atual.

- **Play:** captura o estado refletido da cena e inicia o gameplay (`120/120 → 90/100` na demo).
- **Pause:** pausa o gameplay sem parar a câmera, a renderização ou a interface.
- **Stop:** descarta as alterações no ramo da cena e restaura o snapshot (`120/120`).

O snapshot é em memória, não inclui os recursos globais nem entidades fora do
ramo da cena, e não desfaz alterações no disco. Componentes sem reflexão não
fazem parte do snapshot. O teste `cargo test -p editor` cobre a restauração da
hierarquia e dos componentes refletidos após pausar e retomar.

Em **Edit**, crie ou remova entidades pela Hierarchy. O Inspector lê componentes
registrados no `AppTypeRegistry`; pode adicionar tipos com `ReflectDefault`,
remover componentes e editar campos escalares, texto, booleanos e rotação.
Coleções indexadas expõem seus itens, sem inserir/remover itens; enums e outros
tipos sem controle específico são somente leitura.

Selecione uma entidade com `Transform`, escolha **Mover**, **Girar** ou **Escalar**
e arraste um eixo ou anel com o botão esquerdo na Viewport. A translação usa os
eixos globais; rotação e escala usam os eixos locais. O menu **Snap** alterna
incrementos de 0,5 unidade, 15° e 0,1×, respectivamente. O menu de visualização
permite ocultar a grade e redefinir a câmera.

Na demo, use o **Asset Browser** para selecionar `player_config.ron` e editar seus campos no Inspector,
inclusive durante **Play**. O editor salva após 350 ms sem alterações ou no botão
**Salvar agora**. O watcher do Bevy recarrega o arquivo; mudar `vida_maxima` afeta
a vida e a cor do cubo durante Play. Se o arquivo tiver sido alterado externamente
desde a abertura do rascunho, descarte-o e releia antes de salvar.

O navegador indexa `assets/` e `scenes/` na abertura do editor e exibe cartões
de arquivo com sua extensão. Não gera miniaturas renderizadas de meshes ou materiais.
O **Console** mantém até 500 registros da sessão atual, com busca, filtro por
nível e controle de rolagem automática. A aba **Timeline** mostra as transições
de simulação, não um perfil de desempenho.

## Grafo de fluxos — Fase 4

O **Flow Graph** pertence ao modo **Scripting**, onde ocupa o lugar da Viewport;
ele não aparece no modo Building. Na demo, o grafo abre em `Update` com o filtro
`GamePlugin`. Escolha a vista pelos ícones **Schedule**, **Queries** e
**Observers** na barra esquerda e filtre por plugin ou `SystemSet`. Use a roda
para dar zoom, arraste o fundo com o botão direito ou do meio para navegar e
arraste os nós com o botão esquerdo. As posições são salvas exclusivamente em
`.editor/graph_layout.ron`; **⋮ → Resetar nós** limpa a disposição da vista atual.
Selecionar um nó mostra seus acessos e origem no Inspector.

- **Schedule:** a linha laranja indica ordem declarada no `ScheduleGraph`;
  a cinza indica pertencimento a um conjunto.
- **Queries:** azul indica leitura e roxo indica escrita. Esses acessos não
  garantem ordem nem provam fluxo de dados. Leituras comuns de `Transform`,
  `Time` e `AssetServer` ficam ocultas até ativar **Leituras comuns**.
  `Commands` é um parâmetro deferido, não um acesso de leitura enumerado.
- **Observers:** mostra assinaturas reais `Evento → Observer` e os alvos
  registrados. Na demo, `Damage → apply_damage`. Não atribui o disparo a um
  sistema porque o agendador não expõe o emissor de `trigger`.

O grafo inspeciona o mundo principal da aplicação, sem executar schedules que
ainda não rodaram. Os acessos aparecem após a inicialização de cada schedule.
O filtro de origem distingue `GamePlugin`, `EguiPlugin`, os sistemas do editor
e `DefaultPlugins` como grupo. Registros fora desses pontos aparecem como
origem não registrada.

## Launcher e demo

O launcher do workspace é uma ferramenta de teste: carrega a cena declarada em
`project.ron` e roda sem janela. Não é necessário para rodar ou distribuir os jogos
novos. O gameplay da demo é habilitado em projetos com
`assets/configs/player_config.ron`.

```sh
cargo run -p game_launcher -- ../meu-jogo
cargo run -p game_launcher -- examples/demo --verify-demo
```

Com `--verify-demo`, o launcher espera até 5 segundos pelo resultado
`Health: 90/100` e encerra sozinho. A verificação cobre o carregamento inicial,
não a edição nem o hot reload.

## Linkagem dinâmica opcional

Para usar a linkagem dinâmica no build de desenvolvimento do editor:

```sh
cargo run -p editor --features dev-dynamic
```

Features do Cargo são opt-in, não selecionadas por perfil. `dev-dynamic` não é padrão
e não é necessário para o launcher.
