cd ~/Downloads/EVA_ethereum

for f in config/exection/genesis.json config/execution/genesis.json genesis/genesis.json; do
    if [ -f "$f" ]; then
        echo "===== $f ====="
        jq '{
          chainId: .config.chainId,
          terminalTotalDifficulty: .config.terminalTotalDifficulty,
          difficulty: .difficulty,
          gasLimit: .gasLimit,
          alloc_count: (.alloc | length)
        }' "$f"
    fi
done