# PiraCutter

Transforma uma imagem plana em duas peças prontas para imprimir: um cortador
de biscoitos e o carimbo que marca o desenho na massa.

É um aplicativo de desktop com prévia em 3D, e também funciona pela linha de
comando para trabalhos em lote. Roda tudo localmente, nada é enviado para
lugar nenhum e não tem inteligência artificial envolvida. O contorno é traçado
a partir da imagem e afastado com um campo de distâncias, então a mesma imagem
sempre dá a mesma geometria.

## O que sai

Dê a ele uma ilustração plana com fundo liso, do tipo clipart ou logotipo.

- **Cortador** — a lâmina apoiada em uma aba onde você faz pressão com a mão.
- **Carimbo** — uma placa que entra dentro do cortador, com o contorno em
  relevo e todas as linhas e manchas escuras do desenho.

As duas peças saem como superfícies fechadas, então o fatiador aceita sem
passo de reparo.

## Formatos

**3MF** é o padrão e sai em um único arquivo, com as duas peças nomeadas
Cortador e Carimbo, compactado. Costuma ficar em menos da metade do tamanho
dos STL equivalentes e o fatiador abre as duas peças de uma vez.

**STL** salva um arquivo por peça, terminados em `_cortador.stl` e
`_carimbo.stl`, para fatiadores mais antigos.

## Instalação

Precisa do [Rust](https://rustup.rs). Os mesmos comandos no macOS, Windows e
Linux.

```
cargo build --release
```

O executável fica em `target/release/piracutter` (`piracutter.exe` no
Windows).

No Linux também são necessários os cabeçalhos do sistema de janelas, por
exemplo `libxkbcommon-dev libgtk-3-dev` no Debian e no Ubuntu.

## Uso

Dê um duplo clique no executável, ou rode sem argumentos, para abrir o
aplicativo. Abra uma imagem ou solte ela na janela, ajuste os controles e
clique em Exportar. As configurações ficam salvas entre sessões, e as
predefinições são salvas em JSON.

Para trabalhos em lote:

```
piracutter onca.png -o onca.3mf
piracutter onca.png -o onca.stl
piracutter onca.png -o onca.3mf --size 65
piracutter onca.png -o onca.3mf --config festa.json
```

A extensão do arquivo de saída escolhe o formato.

## Visualização

A vista **3D** mostra as duas peças sólidas sobre a mesa de impressão. Arraste
para girar, arraste com o botão direito para deslocar, use a roda do mouse
para aproximar e dê um clique duplo para enquadrar. Desligue **lado a lado**
para ver o carimbo encaixado dentro do cortador, do jeito que as peças se
encaixam.

A vista **Contornos** mostra os traçados por cima da imagem segmentada, útil
para entender o que foi lido como fundo e o que virou detalhe.

## Configurações que valem a pena conhecer

**Tamanho do biscoito** é medido no desenho, não no arquivo de imagem, então o
espaço vazio ao redor da imagem não encolhe o resultado. Escolha se vale para
a largura, para a altura ou para o maior lado.

**Espelhar geometria** já vem ligado e deve continuar assim. O carimbo é
pressionado virado para baixo, então a peça impressa precisa ser a imagem
espelhada para o biscoito sair na posição certa.

**Espessura da lâmina** em 0,8 mm imprime duas paredes limpas com bico de
0,4 mm. **Engrossar detalhe** conta principalmente em desenhos carregados:
linhas em relevo com menos de 0,8 mm não aguentam a impressão, então engrosse
até aguentarem.

**Suavização** arredonda o contorno antes de traçar. Sem ela o traçado carrega
a escadinha dos pixels da imagem e a silhueta fica com quebras visíveis, mesmo
com o desvio sendo pequeno. No padrão de 0,25 mm a quebra entre segmentos cai
de cerca de 8,5° para 4,5°, e pontas de verdade como chifres e orelhas
continuam no lugar. Passar de 0,5 mm começa a arredondar essas pontas.

**Resolução** controla o passo do traçado. O contorno é encaixado na curva com
precisão de subpixel, então no padrão de 12 px/mm a peça sai a 0,005 mm do
tamanho do desenho. Subir para 16 ou 20 px/mm ganha pouco e deixa a prévia mais
lenta.

**Fundo** é lido pela transparência da imagem quando ela existe, e caso
contrário pela cor ao redor das bordas. Suba a tolerância se sobrarem pedaços
de fundo, baixe se o desenho estiver sendo comido.

Se a barra de estado falar em formas deixadas de fora, algum detalhe ficou
emaranhado demais para virar sólido. Subir a resolução, engrossar o detalhe ou
subir a área mínima do detalhe costuma resolver.

## Impressão

Imprima as duas peças deitadas na mesa, sem suportes. O cortador pede zero
camadas sólidas no topo e dois ou três perímetros. Sobre segurança alimentar
vale o de sempre para cortadores impressos: use filamento próprio ou selado,
lave à mão e trate as peças como utensílio de ocasião, não como louça de
máquina.

## Testes

```
cargo test
```

A suíte gera as duas peças em dezoito combinações de configuração e em
quatorze resoluções, confere que a suavização não come as pontas, lê o 3MF de
volta e falha se alguma superfície exportada não ficar fechada.

## Licença

MIT.
