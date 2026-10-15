# Relatório de alterações — `match` como statement e execução de ADTs

## 1. Resumo executivo

A alteração muda `match` de expressão para statement e implementa o caminho
necessário para usá-lo com valores de tipos algébricos (ADT): parse, validação
semântica, execução e testes. Os braços exigem blocos; o type checker exige
cobertura exaustiva inclusive para padrões aninhados; bindings de padrões
ficam limitados ao bloco do braço.

**Peso para o projeto: alto.** A mudança atravessa a AST e três fases
principais do compilador. Ela altera a gramática pública da linguagem e
acrescenta regras semânticas novas. Não é apenas uma mudança de sintaxe.

## 2. Decisões e justificativas

| Decisão | Justificativa e efeito |
|---|---|
| `match` é statement, não expressão | Evita que a construção prometa um valor resultante e elimina usos como `int x = match (...)` ou `return match (...)`. A execução passa a pertencer a `exec_stmt`, não a `eval_expr`. |
| O scrutinee continua entre parênteses | Mantém o delimitador claro entre a expressão analisada e o início dos braços, e evita que o parser do scrutinee consuma a abertura do bloco. |
| Cada braço exige `=> { ... }`; não há `;` depois do bloco | Mantém o corpo coerente com os statements compostos existentes (`if`, `while` e bloco), permite mais de uma instrução e fornece um escopo léxico explícito para os bindings. |
| Cobertura exaustiva obrigatória, inclusive aninhada | Faz com que cada valor válido do ADT tenha braço selecionável antes da execução. Um erro de ausência de braço em runtime passa a indicar quebra de uma invariável, em vez de ser o mecanismo normal de detecção. |
| Construtores ADT incluídos | Sem construir valores (`Some(42)`, `None`), não há como verificar nem executar `match` de ponta a ponta. O suporte incluído é o mínimo necessário para isso. |
| Geração TAC fora do escopo | O backend existente é parcial e não tem representação/instruções para criar valores ADT e testar/destruturar padrões. Expandir o IR TAC seria outra alteração de arquitetura e excede a migração de `match`. |

## 3. Alterações por área e peso

| Área / arquivos | Alteração | Peso |
|---|---|---|
| AST — [`src/ir/ast.rs`](../src/ir/ast.rs) | `Expr::Match` removido; `Statement::Match` adicionado. `MatchArm.body` agora guarda `StatementD<Ty>` em vez de `ExprD<Ty>`. | **Alto** — define a nova estrutura usada por parser, semântica e execução. |
| Parser — [`src/parser/patterns.rs`](../src/parser/patterns.rs), [`src/parser/statements.rs`](../src/parser/statements.rs), [`src/parser/expressions.rs`](../src/parser/expressions.rs), [`src/parser/functions.rs`](../src/parser/functions.rs), [`src/parser/mod.rs`](../src/parser/mod.rs) | O parser constrói `Statement::Match` e exige blocos nos braços; `match` foi removido de `atom()`. Construtores de nome iniciado por maiúscula são reconhecidos em expressões; identificadores de tipos nomeados são aceitos em declarações, parâmetros e retornos. | **Alto** — muda a gramática e habilita a sintaxe ADT no restante da linguagem. |
| Type checker — [`src/semantic/type_checker.rs`](../src/semantic/type_checker.rs) | Registro e validação dos ADTs, checagem de construtores, padrões, escopos, tipos e exaustividade; detalhes na seção 4. | **Muito alto** — determina quais programas são aceitos e garante as precondições do interpretador. |
| Runtime — [`src/interpreter/value.rs`](../src/interpreter/value.rs), [`src/interpreter/eval_expr.rs`](../src/interpreter/eval_expr.rs), [`src/interpreter/exec_stmt.rs`](../src/interpreter/exec_stmt.rs) | `Value::Adt` representa uma variante e seus campos; construtores materializam valores; `match` seleciona o primeiro padrão compatível, instala os bindings temporariamente e executa o bloco escolhido. Igualdade recursiva de valores ADT também foi acrescentada. | **Alto** — entrega comportamento executável e preserva propagação de retorno. |
| Testes — [`src/parser/patterns.rs`](../src/parser/patterns.rs), [`tests/parser.rs`](../tests/parser.rs), [`tests/type_checker.rs`](../tests/type_checker.rs), [`tests/interpreter.rs`](../tests/interpreter.rs) | Casos de sintaxe, rejeição em contexto de expressão, payloads, cobertura incompleta/aninhada, variante incompatível, escopo e execução com `return`. | **Alto** — protege a integração entre fases e as novas invariantes semânticas. |
| Documentação — [`docs/12-sintaxe-adt.md`](./12-sintaxe-adt.md), [`docs/13-plano-trabalho-adt.md`](./13-plano-trabalho-adt.md), [`docs/14-apresentacao-parser-pattern-matching.md`](./14-apresentacao-parser-pattern-matching.md), [`docs/15-slides-pessoa3-parser-match.md`](./15-slides-pessoa3-parser-match.md), [`docs/16-roteiro-apresentacao-pessoa3.md`](./16-roteiro-apresentacao-pessoa3.md), [`docs/17-todo-match-como-statement.md`](./17-todo-match-como-statement.md) | Sintaxe atual documentada; plano e materiais que descrevem a proposta anterior identificados como históricos; TODO marcado como concluído. | **Médio** — reduz contradições para quem consulta a especificação e o histórico. |

## 4. Type checker — alterações, justificativas e peso

Esta seção descreve exclusivamente o trabalho realizado no type checker.

### 4.1 Tabela semântica dos tipos algébricos

Foi introduzida uma estrutura interna `TypeInfo` com:

- um mapa de nomes de ADT para suas declarações;
- um mapa de nome de variante para o ADT proprietário e os tipos dos campos.

O mapa é construído uma vez por chamada de `type_check`, antes de verificar
os corpos das funções. Isso evita reconstruir a relação entre variante,
proprietário e payload em cada expressão ou padrão, e dá aos passes
subsequentes uma fonte comum de validação.

Na construção, o checker rejeita:

- declarações ADT duplicadas;
- ADTs sem variantes;
- nomes de variantes duplicados globalmente;
- campos de variante do tipo `void`;
- tipos nomeados não declarados, inclusive em payloads de variantes.

A unicidade global de variantes é necessária nesta implementação porque o
nome do construtor é resolvido sem qualificação pelo contexto e armazenado
num mapa indexado por esse nome. Permitir o mesmo nome em ADTs distintos
exigiria outra regra de resolução e potencialmente qualificação explícita.
**Essa é uma restrição semântica adicionada por esta implementação**, não
uma consequência inevitável de ADTs em geral.

### 4.2 Validação de tipos nomeados

O checker percorre os tipos de retorno e parâmetros das funções, os tipos de
declaração local e os campos das variantes. `Type::Named(name)` só é aceito
se `name` corresponder a um ADT declarado; arrays são percorridos
recursivamente.

Isso é importante porque a parser reconhece a forma lexical de um tipo
nomeado, mas não tem informação sobre declarações do programa. Deixar essa
decisão para o type checker mantém a separação entre sintaxe e semântica e
evita que um nome inexistente chegue ao interpretador.

### 4.3 Checagem de expressões construtoras

Para cada `Expr::Construct`, o checker:

1. procura a variante na tabela semântica;
2. compara a quantidade de argumentos com a quantidade de campos;
3. verifica o tipo de cada argumento contra o tipo do campo correspondente,
   usando as regras existentes de compatibilidade;
4. atribui à expressão construtora o tipo `Type::Named` do ADT proprietário;
5. reconstrói a expressão tipada, anotando recursivamente seus argumentos.

Esse caminho permite que a construção seja usada em inicializações,
argumentos de função e outras expressões normais. Também rejeita construtores
desconhecidos, payloads de aridade incorreta e payloads de tipo incompatível
antes da execução.

### 4.4 Checagem dos statements `match`

Para cada `Statement::Match`, o checker:

1. verifica e anota o tipo do scrutinee;
2. exige que o tipo seja `Type::Named` de um ADT conhecido;
3. exige que o corpo de cada braço seja um bloco;
4. valida cada padrão em relação ao tipo esperado;
5. cria os bindings tipados para o corpo daquele braço;
6. verifica cada bloco com o tipo de retorno corrente da função;
7. restaura o ambiente ao snapshot anterior ao braço;
8. verifica a cobertura exaustiva de todos os padrões.

O bloco obrigatório é validado também semanticamente. Isso torna a regra
explícita mesmo para ASTs construídas diretamente por outros chamadores, sem
depender exclusivamente do parser.

### 4.5 Validação de padrões e bindings

`check_pattern` recebe o tipo esperado do valor e valida recursivamente:

- `Wildcard` é válido e não cria binding;
- `Bind(name)` associa o nome ao tipo esperado;
- `Variant { name, fields }` exige um tipo ADT, uma variante conhecida
  pertencente exatamente àquele ADT e a aridade correta;
- cada subpadrão é checado contra o tipo declarado do campo correspondente;
- um mesmo nome não pode ser vinculado duas vezes no mesmo padrão.

Um binding que captura o valor inteiro herda o tipo do scrutinee; um binding
dentro de uma variante herda o tipo do campo declarado. Assim, operações e
chamadas dentro do braço são verificadas com tipos reais e não com `Any`.

Antes de instalar os bindings, o checker rejeita colisão com nomes já visíveis
no ambiente. Depois de verificar o braço, restaura o snapshot, impedindo que
bindings de um braço vazem para o próximo ou para fora do `match`. A checagem
do corpo também recebe `expected_return`, de modo que `return` dentro do
braço segue as mesmas regras da função envolvente.

### 4.6 Cobertura exaustiva recursiva

A cobertura é verificada por `patterns_exhaustive` e
`is_matrix_exhaustive`, usando uma matriz de padrões e os tipos das colunas:

- para um ADT, especializa a matriz uma vez para cada variante declarada;
- um padrão da variante correspondente contribui seus subpadrões;
- wildcard e binding expandem para wildcards dos campos dessa variante;
- os tipos dos campos tornam-se as próximas colunas e são analisados
  recursivamente;
- para tipos não ADT, só wildcard/binding cobrem todos os valores possíveis;
- o match só é exaustivo se todas as variantes e recursões forem cobertas.

Assim, cobrir `Wrapped(Left)` não cobre automaticamente
`Wrapped(Right)`. Um wildcard ou binding no nível apropriado cobre os casos
restantes. A análise ocorre depois da validação dos padrões e dos corpos:
isso evita que um padrão inválido ou um corpo mal tipado seja mascarado por
um diagnóstico de não exaustividade.

**Peso: muito alto.** Esta parte transforma `match` em construção confiável
em tempo de compilação: sem cobertura, pertencimento correto das variantes,
aridades e tipos dos bindings, o runtime poderia receber padrões impossíveis
ou precisar tratar silenciosamente valores sem braço correspondente.

### 4.7 Compatibilidade entre valores do mesmo ADT

`types_compatible` agora aceita `Type::Named(a)` e `Type::Named(b)` apenas
quando os nomes são iguais. Isso habilita atribuição, declaração,
parâmetros e retornos do mesmo ADT, sem permitir mistura de tipos algébricos
distintos.

## 5. Comportamento em runtime

`Value::Adt` armazena o tipo proprietário, o nome da variante e seus valores
de payload. `eval_expr` avalia os argumentos de `Expr::Construct` e monta
essa representação. A anotação de tipo produzida pelo checker fornece o
nome do ADT proprietário.

Em `exec_stmt`, o scrutinee é avaliado uma vez. Os braços são percorridos na
ordem do fonte até encontrar padrão compatível; o braço é então executado
com seus bindings no ambiente. Ao terminar — com sucesso, erro ou retorno —
os bindings locais são removidos, mantendo alterações a variáveis externas e
propagando o resultado do statement. Se nenhum braço casar, o runtime devolve
erro explícito indicando violação da invariável de exaustividade.

A igualdade de valores ADT compara tipo, variante e payload recursivamente.
Isso mantém coerência quando valores ADT são comparados por `==`/`!=`.

## 6. Testes e validação

Foram acrescentados testes para:

- parsing de construtores com e sem payload;
- parsing de `match` como statement e rejeição como expressão;
- exigência de blocos nos braços;
- construção válida, payload de tipo errado e variante desconhecida;
- cobertura exaustiva e rejeição de cobertura incompleta;
- exaustividade recursiva para padrões aninhados;
- rejeição de variante pertencente a outro ADT;
- isolamento de bindings fora do braço;
- seleção de variante, execução de construtores aninhados e propagação de
  `return`.

Validação executada:

- `cargo check`: passou;
- `cargo test --lib --test parser --test type_checker --test interpreter --test program --test stdlib`: passou — 145 testes;
- `cargo test`: falhou somente em
  `tests/tac_gen.rs::test_if_else_with_relational_condition`. O teste esperava
  uma sequência/ordem de instruções TAC diferente da produzida. Essa falha
  pertence ao backend TAC, explicitamente fora do escopo desta alteração.

O build emite warnings de funções TAC não utilizadas e um import `Literal`
não usado em `tests/tac_gen.rs`. Esses avisos não impediram a compilação nem
os testes selecionados.

## 7. Limitações e impacto futuro

- O gerador TAC não foi atualizado para construtores nem para `match`; a
  geração de código para programas que usam ADTs requer uma etapa própria.
- Os nomes das variantes devem ser globalmente únicos na implementação atual.
- Não foram incluídos padrões literais, guards nem desestruturação de arrays.
- A cobertura exaustiva considera ADTs e seus payloads recursivamente; para
  tipos não ADT, apenas wildcard/binding são abrangentes.

Em síntese, a mudança tem impacto alto no front-end e no interpretador, mas
não altera o formato TAC nem outros backends. A suite focada nas fases
implementadas passa integralmente; o resultado do conjunto completo fica
limitado pelo teste TAC descrito acima.
