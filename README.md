# Engine Visual Nativa para Bevy

Editor nativo em **Bevy 0.20.1**. A partir da raiz do workspace:

```sh
cargo run -p editor
cargo run -p game_launcher
bash scripts/test-ejection.sh
```

## Editor

Os painéis podem ser movidos, redimensionados e reacoplados pelas abas.
Em `Editors → Restaurar layout inicial` é possível voltar à disposição padrão.
O tema Warm Charcoal usa três colunas: Hierarchy e Asset Browser à esquerda,
Viewport sobre Flow Graph e Console no centro, e Inspector à direita.
Na Viewport, arraste com o botão direito para olhar, use WASD/QE durante o arrasto
para mover e a roda para aproximar ou afastar. O cubo é uma prévia 3D da entidade
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

No **Asset Browser**, selecione `player_config.ron` para editar seus campos no Inspector,
inclusive durante **Play**. O editor salva após 350 ms sem alterações ou no botão
**Salvar agora**. O watcher do Bevy recarrega o arquivo; mudar `vida_maxima` afeta
a vida e a cor do cubo durante Play. Se o arquivo tiver sido alterado externamente
desde a abertura do rascunho, descarte-o e releia antes de salvar.

O navegador indexa `assets/` e `scenes/` na abertura do editor e exibe cartões
de arquivo com sua extensão. Não gera miniaturas renderizadas de meshes ou materiais.
O **Console** mantém até 500 registros da sessão atual, com busca, filtro por
nível e controle de rolagem automática. A aba **Timeline** mostra as transições
de simulação, não um perfil de desempenho. No Linux, o editor usa X11 ou XWayland.

## Grafo de fluxos — Fase 4

O **Flow Graph** fica abaixo da **Viewport**. O grafo abre em
`Update` com o filtro `GamePlugin`. Troque o
schedule e filtre por plugin ou `SystemSet`. Use a roda para dar zoom, arraste
o fundo com o botão direito ou do meio para navegar e arraste os nós com o
botão esquerdo. As posições são salvas exclusivamente em
`.editor/graph_layout.ron`; **⋮ → Resetar nós** limpa a disposição da vista atual.

- **Agendador:** a linha laranja indica ordem declarada no `ScheduleGraph`;
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

## Launcher e teste de ejeção

O launcher da Fase 1 roda sem janela e encerra sozinho. Ele carrega `scenes/level.scn.ron`
pela API nativa `DynamicWorld`/`DynamicWorldRoot`, lê `assets/configs/player_config.ron` pelo
`AssetServer` e passa dano ao jogador via `EntityEvent` e `On<Damage>`. O resultado esperado
é `Health: 90/100`: a cena começa com 120, a config limita a vida a 100 e o Observer
subtrai 10. Se o resultado não aparecer em 5 segundos, o launcher retorna erro.

O teste de ejeção executa o mesmo launcher numa cópia temporária sem `crates/editor/`,
sem `.editor/` e sem o membro `editor` no manifesto. O workspace original não é alterado.

O `file_watcher` está habilitado, mas o smoke test valida somente o carregamento inicial,
não o ciclo de edição/hot-reload.

## Linkagem dinâmica opcional

Para usar a linkagem dinâmica no build de desenvolvimento do editor:

```sh
cargo run -p editor --features dev-dynamic
```

Features do Cargo são opt-in, não selecionadas por perfil. `dev-dynamic` não é padrão
e não é necessário para o launcher.
