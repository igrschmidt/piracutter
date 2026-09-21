Programas prontos para usar. Não precisa instalar Rust, nem compilador, nem
nada: baixe, descompacte e abra.

## Windows

Baixe `PiraCutter-windows-x64.zip`, descompacte e dê um duplo clique em
`piracutter.exe`. O executável é autocontido, não precisa do Visual C++
Redistributable. Roda em Windows 10 e 11, 64 bits.

O Windows pode mostrar um aviso azul "O Windows protegeu o computador", porque
o programa não tem assinatura digital paga. Clique em **Mais informações** e
depois em **Executar assim mesmo**.

## macOS

Baixe `PiraCutter-macos-apple-silicon.zip`, descompacte e arraste
`PiraCutter.app` para onde quiser. É para Mac com chip Apple (M1 em diante), a
partir do macOS 11. Mac com Intel não é atendido: compile do código se
precisar.

Na primeira vez, clique com o botão direito no aplicativo e escolha **Abrir**,
e confirme no aviso. Um duplo clique normal vai ser bloqueado, porque o
aplicativo não é assinado por uma conta paga de desenvolvedor da Apple. Depois
da primeira vez, abre normal. Se preferir pela linha de comando:

```
xattr -dr com.apple.quarantine /caminho/para/PiraCutter.app
```

## Linha de comando

O mesmo programa serve para trabalhos em lote. No Mac o executável fica dentro
do pacote, em `PiraCutter.app/Contents/MacOS/piracutter`.

```
piracutter vaca.png -o vaca.3mf
piracutter vaca.png -o vaca.3mf --size 65
```
