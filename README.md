# PkGUI

> Software Manager para BSD com ferramentas nativas

## Descrição

PkGUI é uma interface gráfica simples para gestão de pacotes em sistemas BSD (FreeBSD, OpenBSD, etc.) utilizando ferramentas nativas como `pkg` e `ports`.

## Funcionalidades

- [ ] Listar pacotes instalados
- [ ] Pesquisar pacotes disponíveis
- [ ] Instalar pacotes
- [ ] Remover pacotes
- [ ] Atualizar sistema
- [ ] Ver informações de pacotes

## Requisitos

- FreeBSD 13+ (ou outro sistema BSD)
- GTK 4.x
- Rust 1.70+
- `pkg` (gestor de pacotes do FreeBSD)

### Dependências Rust

```toml
[dependencies]
gtk4 = "0.8"
libadwaita = "0.5"
```

## Instalação

### Em FreeBSD

```sh
# Instalar dependências
sudo pkg install rust gtk4 libadwaita

# Clonar e compilar
git clone https://github.com/Olen1962/pkgui.git
cd pkgui
cargo build --release

# Executar
./target/release/pkgui
```

## Desenvolvimento

```sh
# Executar em modo desenvolvimento
cargo run

# Compilar para release
cargo build --release
```

## Estrutura do Projeto

```
pkgui/
├── Cargo.toml          # Configuração do projeto Rust
├── src/
│   └── main.rs        # Código principal
├── LICENSE            # Licença MIT
└── README.md          # Documentação
```

## Contribuir

1. Faz fork do repositório
2. Cria uma branch para a tua feature (`git checkout -b feature/nova-feature`)
3. Faz commit das tuas alterações (`git commit -m 'Adiciona nova feature'`)
4. Faz push para a branch (`git push origin feature/nova-feature`)
5. Abre um Pull Request

## Licença

MIT License - veja o ficheiro [LICENSE](LICENSE) para mais detalhes.
