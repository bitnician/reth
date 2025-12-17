#!/bin/bash

# Test for --txpool.preserve-insertion-order flag (NO DEV MODE)
# This version checks mempool ordering directly

# Configuration
RPC_URL="http://localhost:8545"
PRIVATE_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
FROM_ADDR="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
TO_ADDR="0x70997970C51812dc3A010C7d01b50e0d17dc79C8"

# Get current nonce
NONCE=$(cast nonce $FROM_ADDR --rpc-url $RPC_URL 2>/dev/null || echo "0")
echo "Starting nonce: $NONCE"
echo ""

# Send 3 transactions with different gas prices
echo "Sending 3 transactions in order: LOW → HIGH → MEDIUM gas"
echo ""

# TX1: LOW gas (1 gwei)
echo "  [1] Sending LOW gas tx (1 gwei)..."
TX1=$(cast send --rpc-url $RPC_URL \
    --private-key $PRIVATE_KEY \
    --nonce $NONCE \
    --gas-price 1gwei \
    $TO_ADDR --value 0.001ether \
    --async 2>/dev/null)

# TX2: HIGH gas (10 gwei)
echo "  [2] Sending HIGH gas tx (10 gwei)..."
TX2=$(cast send --rpc-url $RPC_URL \
    --private-key $PRIVATE_KEY \
    --nonce $(($NONCE + 1)) \
    --gas-price 10gwei \
    $TO_ADDR --value 0.002ether \
    --async 2>/dev/null)

# TX3: MEDIUM gas (5 gwei)
echo "  [3] Sending MEDIUM gas tx (5 gwei)..."
TX3=$(cast send --rpc-url $RPC_URL \
    --private-key $PRIVATE_KEY \
    --nonce $(($NONCE + 2)) \
    --gas-price 5gwei \
    $TO_ADDR --value 0.003ether \
    --async 2>/dev/null)

echo ""
echo "Transactions sent. Checking mempool..."
sleep 1

# Check mempool content and order
echo ""
echo "=== MEMPOOL ORDER ==="
echo ""

# Get pending transactions
POOL_CONTENT=$(curl -s -X POST $RPC_URL \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"txpool_content","params":[],"id":1}' | jq '.result.pending')

if [ "$POOL_CONTENT" = "null" ] || [ "$POOL_CONTENT" = "{}" ]; then
    echo "❌ No transactions in mempool. Make sure Reth is running without dev mode."
    exit 1
fi

# Extract and show transaction order
echo "Transactions in mempool (execution order):"
echo "-------------------------------------------"

POSITION=1
FOUND_ORDER=""

# Parse the pending pool - transactions are returned in execution order
for sender in $(echo "$POOL_CONTENT" | jq -r 'keys[]' 2>/dev/null); do
    for nonce in $(echo "$POOL_CONTENT" | jq -r ".\"$sender\" | keys[]" 2>/dev/null); do
        TX_DATA=$(echo "$POOL_CONTENT" | jq ".\"$sender\".\"$nonce\"")
        
        # Get value to identify transaction (using value since gas price varies)
        VALUE=$(echo "$TX_DATA" | jq -r '.value')
        TX_HASH=$(echo "$TX_DATA" | jq -r '.hash')
        GAS_PRICE=$(echo "$TX_DATA" | jq -r '.gasPrice // .maxFeePerGas')
        
        # Identify which transaction based on value
        # 0x38d7ea4c68000 = 0.001 ether (LOW gas)
        # 0x71afd498d0000 = 0.002 ether (HIGH gas)  
        # 0xaa87bee538000 = 0.003 ether (MEDIUM gas)
        
        LABEL=""
        if [ "$VALUE" = "0x38d7ea4c68000" ]; then
            LABEL="LOW (1 gwei)"
            FOUND_ORDER="${FOUND_ORDER}LOW "
        elif [ "$VALUE" = "0x71afd498d0000" ]; then
            LABEL="HIGH (10 gwei)"
            FOUND_ORDER="${FOUND_ORDER}HIGH "
        elif [ "$VALUE" = "0xaa87bee538000" ]; then
            LABEL="MEDIUM (5 gwei)"
            FOUND_ORDER="${FOUND_ORDER}MEDIUM "
        fi
        
        echo "  Position $POSITION: $LABEL - ${TX_HASH:0:10}..."
        POSITION=$((POSITION + 1))
    done
done

echo ""
echo "=== RESULTS ==="
echo ""
echo "Actual order: $FOUND_ORDER"
echo ""

if [ "$FOUND_ORDER" = "LOW HIGH MEDIUM " ]; then
    echo "✅ INSERTION ORDER PRESERVED!"
    echo "   Transactions are in submission order (ignoring gas prices)"
    echo "   The --txpool.preserve-insertion-order flag is WORKING!"
elif [ "$FOUND_ORDER" = "HIGH MEDIUM LOW " ]; then
    echo "✅ GAS PRICE ORDERING (Normal behavior)"
    echo "   Transactions are sorted by gas price (highest first)"
    echo "   This is the default behavior without the flag"
else
    echo "⚠️  Unexpected order: $FOUND_ORDER"
    echo "   Expected either:"
    echo "   - 'LOW HIGH MEDIUM' (with --txpool.preserve-insertion-order)"
    echo "   - 'HIGH MEDIUM LOW' (without flag)"
fi

echo ""
echo "=== CLEANUP ==="
echo "To clear the mempool, you can:"
echo "1. Restart Reth"
echo "2. Or mine a block manually if you have mining setup"