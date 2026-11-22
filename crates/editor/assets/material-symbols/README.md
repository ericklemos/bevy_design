# Material Symbols Outlined

Fonte estática oficial do Google (versão 2.983), com `opsz=20`, `wght=400`,
`FILL=0` e `GRAD=0`. O subconjunto vem da API do Google Fonts e é embutido
no editor; não é um asset dos projetos.

Copyright 2026 Google LLC. Distribuída sob a [Apache License 2.0](LICENSE).

Para incluir um ícone:

1. Consulte o [catálogo de codepoints](https://github.com/google/material-design-icons/blob/master/variablefont/MaterialSymbolsOutlined%5BFILL,GRAD,opsz,wght%5D.codepoints)
   e adicione o símbolo ao enum `Icon` e ao teste em [`src/icons.rs`](../../src/icons.rs).
2. Acrescente o nome oficial em `icon_names` abaixo, mantendo a lista em ordem
   alfabética. Baixe o URL `format('truetype')` retornado e salve como
   `MaterialSymbolsOutlined.ttf` nesta pasta.
3. Execute `cargo test -p editor icons::tests` para confirmar que todos os
   símbolos registrados continuam disponíveis.

```sh
curl -fsSL 'https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@20,400,0,0&icon_names=account_tree,chevron_right,close,deployed_code,desktop_windows,expand_more,filter_alt,flowchart,folder,lock,lock_open,more_vert,pause,person,play_arrow,schedule,sort,stop,terminal,tune,visibility,visibility_off'
```
