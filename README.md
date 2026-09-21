# PiraCutter

Transforma uma imagem plana em dois ficheiros STL prontos a imprimir: um
cortador de bolachas e o carimbo que marca o desenho na massa.

É uma aplicação de secretária com pré-visualização em 3D, e também funciona
por linha de comandos para trabalhos em série. Corre tudo localmente, nada é
enviado para lado nenhum e não há inteligência artificial pelo meio. O
contorno é traçado a partir da imagem e afastado com um campo de distâncias,
por isso a mesma imagem dá sempre a mesma geometria.

## O que sai

Dê-lhe uma ilustração plana com fundo liso, do género de clipart ou logótipo.

- **Cortador** — a lâmina assente numa aba onde faz pressão com a mão.
- **Carimbo** — uma placa que entra dentro do cortador, com o contorno em
  relevo e todas as linhas e manchas escuras do desenho.

As duas peças são exportadas como superfícies fechadas, por isso o laminador
aceita-as sem passo de reparação.

## Instalação

Precisa do [Rust](https://rustup.rs). Os mesmos comandos em macOS, Windows e
Linux.

```
cargo build --release
```

O executável fica em `target/release/piracutter` (`piracutter.exe` no
Windows).

Em Linux são ainda precisos os cabeçalhos habituais do sistema de janelas, por
exemplo `libxkbcommon-dev libgtk-3-dev` em Debian e Ubuntu.

## Utilização

Faça duplo clique no executável, ou corra-o sem argumentos, para abrir a
aplicação. Abra uma imagem ou largue-a na janela, mexa nos controlos e carregue
em Exportar STL. São gravados dois ficheiros junto do nome que escolher,
terminados em `_cortador.stl` e `_carimbo.stl`. As definições ficam guardadas
entre sessões, e as predefinições gravam-se em JSON.

Para trabalhos em série:

```
piracutter leopardo.png -o leopardo.stl
piracutter leopardo.png -o leopardo.stl --size 65
piracutter leopardo.png -o leopardo.stl --config festa.json
```

## Vistas

A vista **3D** mostra as duas peças sólidas sobre a mesa de impressão. Arraste
para rodar, arraste com o botão direito para deslocar, use a roda do rato para
ampliar e faça duplo clique para enquadrar. Desligue **lado a lado** para ver o
carimbo encaixado dentro do cortador, como as peças assentam uma na outra.

A vista **Contornos** mostra os traçados por cima da imagem segmentada, útil
para perceber o que foi lido como fundo e o que passou a detalhe.

## Definições que vale a pena conhecer

**Tamanho da bolacha** é medido no desenho, não no ficheiro de imagem, por isso
o espaço vazio à volta da imagem não encolhe o resultado. Escolha se se aplica
à largura, à altura ou ao maior lado.

**Espelhar geometria** vem ligado e deve ficar assim. O carimbo é pressionado
virado para baixo, por isso a peça impressa tem de ser a imagem espelhada para
a bolacha sair na posição certa.

**Espessura da lâmina** a 0,8 mm imprime duas paredes limpas com um bico de
0,4 mm. **Engrossar detalhe** conta sobretudo em desenhos carregados: linhas em
relevo com menos de 0,8 mm não aguentam a impressão, por isso engrosse-as até
aguentarem.

**Fundo** é lido a partir da transparência da imagem quando ela existe, e caso
contrário a partir da cor à volta das margens. Suba a tolerância se sobrarem
pedaços de fundo, baixe-a se o desenho estiver a ser comido.

Se a barra de estado falar em formas deixadas de fora, algum detalhe ficou
emaranhado demais para virar sólido. Subir a resolução, engrossar o detalhe ou
subir a área mínima do detalhe costuma resolver.

## Impressão

Imprima as duas peças deitadas na mesa, sem suportes. O cortador quer zero
camadas sólidas no topo e dois ou três perímetros. Quanto a segurança
alimentar vale o costume para cortadores impressos: use filamento próprio ou
selado, lave à mão e trate-os como utensílios de ocasião e não como loiça de
máquina.

## Testes

```
cargo test
```

A bateria gera as duas peças em dezassete combinações de definições e falha se
alguma superfície exportada não ficar fechada.

## Licença

MIT.
