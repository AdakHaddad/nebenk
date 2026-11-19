// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.20;

/// @title NeenEscrow - Decentralized escrow for NEBENK marketplace deployments
/// @author NEBENK Contributors
/// @notice Handles escrow, epoch-based payments, and slashing for marketplace deployments

contract NeenEscrow {
    // ============ Events ============
    event EscrowCreated(
        bytes32 indexed deploymentId,
        address indexed developer,
        address[] providers,
        string token,
        uint256 totalDeposited,
        uint32 epochsFunded,
        uint256 createdAt
    );

    event FundsDeposited(
        bytes32 indexed deploymentId,
        address indexed depositor,
        uint256 amount,
        uint256 newBalance
    );

    event EpochSettled(
        bytes32 indexed deploymentId,
        uint32 indexed epoch,
        address indexed provider,
        uint256 payment,
        uint256 slashed,
        bool slaMet,
        uint256 settledAt
    );

    event ProviderSlashed(
        bytes32 indexed deploymentId,
        address indexed provider,
        uint256 amount,
        string reason
    );

    event WithdrawalRequested(
        address indexed provider,
        uint256 amount,
        uint256 lockExpiry
    );

    event WithdrawalExecuted(
        address indexed provider,
        uint256 amount
    );

    event EscrowClosed(
        bytes32 indexed deploymentId,
        uint256 refundAmount
    );

    event DisputeRaised(
        bytes32 indexed deploymentId,
        address indexed challenger,
        address indexed target,
        string reason
    );

    event DisputeResolved(
        bytes32 indexed deploymentId,
        bool providerWon,
        uint256 compensation
    );

    // ============ State ============
    struct Escrow {
        bytes32 deploymentId;
        address developer;
        address[] providers;
        IERC20 token;
        uint256 totalDeposited;
        uint256 totalPaid;
        uint256 totalSlashed;
        uint256 currentBalance;
        uint32 epochsFunded;
        uint64 createdAt;
        uint64 lastSettlementAt;
        EscrowStatus status;
        mapping(address => uint256) providerStakes;
        mapping(address => uint256) providerSlashes;
    }

    enum EscrowStatus {
        Active,
        Depleted,
        Refunding,
        Closed,
        Disputed
    }

    struct ProviderStake {
        uint256 amount;
        string token;
        bool locked;
        uint64 lockExpiresAt;
        uint256 totalSlashed;
    }

    struct EpochMetrics {
        address provider;
        uint256 uptimePercentage; // Basis points (10000 = 100%)
        uint256 replicationLagMs;
        bool dataCorrupted;
        bool maliciousDetected;
        uint256 operationsProcessed;
        uint256 bytesTransferred;
    }

    struct EpochSettlement {
        uint32 epoch;
        bytes32 deploymentId;
        address provider;
        uint64 epochStart;
        uint64 epochEnd;
        uint256 uptimePercentage;
        uint256 replicationLagMs;
        bool slaMet;
        uint256 paymentAmount;
        uint256 slashedAmount;
        uint64 settledAt;
    }

    // ============ Storage ============
    mapping(bytes32 => Escrow) public escrows;
    mapping(address => ProviderStake) public providerStakes;
    EpochSettlement[] public settlementHistory;

    // Configuration
    uint256 public epochDuration = 3600; // 1 hour default
    uint256 public minUptimePercentage = 9900; // 99.00% in basis points
    uint256 public maxReplicationLagMs = 5000;
    
    // Slashing percentages (basis points)
    uint256 public uptimeSlashPercentage = 1000; // 10%
    uint256 public replicationLagSlashPercentage = 2000; // 20%
    uint256 public dataCorruptionSlashPercentage = 5000; // 50%
    uint256 public maliciousSlashPercentage = 10000; // 100%

    // Escrow config
    uint256 public minDepositEpochs = 2;
    uint256 public autoRefillThreshold = 2000; // 20% in basis points
    uint256 public withdrawalLockEpochs = 24;

    address public owner;
    bool public paused;

    // ============ Modifiers ============
    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }

    modifier onlyDeveloper(bytes32 deploymentId) {
        require(escrows[deploymentId].developer == msg.sender, "Not developer");
        _;
    }

    modifier onlyProvider(bytes32 deploymentId) {
        bool isProvider = false;
        for (uint i = 0; i < escrows[deploymentId].providers.length; i++) {
            if (escrows[deploymentId].providers[i] == msg.sender) {
                isProvider = true;
                break;
            }
        }
        require(isProvider, "Not provider");
        _;
    }

    modifier escrowActive(bytes32 deploymentId) {
        require(escrows[deploymentId].status == EscrowStatus.Active, "Escrow not active");
        _;
    }

    modifier whenNotPaused() {
        require(!paused, "Contract paused");
        _;
    }

    // ============ Constructor ============
    constructor() {
        owner = msg.sender;
    }

    // ============ Admin Functions ============
    function setEpochDuration(uint256 _duration) external onlyOwner {
        require(_duration >= 300, "Epoch too short"); // Min 5 minutes
        epochDuration = _duration;
    }

    function setMinUptime(uint256 _percentage) external onlyOwner {
        require(_percentage <= 10000, "Invalid percentage");
        minUptimePercentage = _percentage;
    }

    function setSlashingPercentages(
        uint256 _uptime,
        uint256 _replicationLag,
        uint256 _dataCorruption,
        uint256 _malicious
    ) external onlyOwner {
        require(_uptime <= 10000, "Invalid uptime slash");
        require(_replicationLag <= 10000, "Invalid replication slash");
        require(_dataCorruption <= 10000, "Invalid data corruption slash");
        require(_malicious <= 10000, "Invalid malicious slash");
        uptimeSlashPercentage = _uptime;
        replicationLagSlashPercentage = _replicationLag;
        dataCorruptionSlashPercentage = _dataCorruption;
        maliciousSlashPercentage = _malicious;
    }

    function pause() external onlyOwner {
        paused = true;
    }

    function unpause() external onlyOwner {
        paused = false;
    }

    // ============ Escrow Functions ============
    /// @notice Create a new escrow for a deployment
    /// @param deploymentId Unique deployment identifier (32 bytes)
    /// @param providers Array of provider addresses
    /// @param token ERC20 token address for payments
    /// @param epochsToFund Number of epochs to pre-fund
    /// @param estimatedCostPerEpoch Estimated cost per epoch in token units
    function createEscrow(
        bytes32 deploymentId,
        address[] calldata providers,
        IERC20 token,
        uint32 epochsToFund,
        uint256 estimatedCostPerEpoch
    ) external whenNotPaused {
        require(escrows[deploymentId].status == EscrowStatus.Active, "Escrow exists");
        require(providers.length > 0, "No providers");
        require(epochsToFund >= minDepositEpochs, "Insufficient epochs");
        require(estimatedCostPerEpoch > 0, "Zero cost");

        uint256 totalDeposit = estimatedCostPerEpoch * epochsToFund;
        
        // Pull funds from developer
        token.safeTransferFrom(msg.sender, address(this), totalDeposit);

        Escrow storage escrow = escrows[deploymentId];
        escrow.deploymentId = deploymentId;
        escrow.developer = msg.sender;
        escrow.providers = providers;
        escrow.token = token;
        escrow.totalDeposited = totalDeposit;
        escrow.currentBalance = totalDeposit;
        escrow.epochsFunded = epochsToFund;
        escrow.createdAt = uint64(block.timestamp);
        escrow.status = EscrowStatus.Active;

        // Initialize provider stakes
        for (uint i = 0; i < providers.length; i++) {
            escrow.providerStakes[providers[i]] = 0;
            escrow.providerSlashes[providers[i]] = 0;
        }

        emit EscrowCreated(
            deploymentId,
            msg.sender,
            providers,
            token.symbol(),
            totalDeposit,
            epochsToFund,
            uint64(block.timestamp)
        );
    }

    /// @notice Deposit additional funds to escrow
    function deposit(bytes32 deploymentId, uint256 amount) external whenNotPaused onlyDeveloper(deploymentId) {
        Escrow storage escrow = escrows[deploymentId];
        require(escrow.status == EscrowStatus.Active, "Escrow not active");
        require(amount > 0, "Zero amount");

        escrow.token.safeTransferFrom(msg.sender, address(this), amount);
        
        escrow.totalDeposited += amount;
        escrow.currentBalance += amount;
        escrow.epochsFunded += 1;

        emit FundsDeposited(deploymentId, msg.sender, amount, escrow.currentBalance);
    }

    /// @notice Settle an epoch for a deployment
    /// @param deploymentId Deployment identifier
    /// @param epoch Epoch number
    /// @param metrics Array of provider metrics for this epoch
    function settleEpoch(
        bytes32 deploymentId,
        uint32 epoch,
        EpochMetrics[] calldata metrics
    ) external whenNotPaused onlyDeveloper(deploymentId) escrowActive(deploymentId) {
        Escrow storage escrow = escrows[deploymentId];
        
        uint64 epochStart = escrow.createdAt + uint64(epoch) * uint64(epochDuration);
        uint64 epochEnd = epochStart + uint64(epochDuration);
        
        require(block.timestamp >= epochEnd, "Epoch not ended");

        uint256 basePayment = escrow.totalDeposited / escrow.epochsFunded;
        uint256 totalPaidThisEpoch = 0;

        for (uint i = 0; i < metrics.length; i++) {
            EpochMetrics memory m = metrics[i];
            
            // Verify provider is in escrow
            require(escrow.providerStakes[m.provider] >= 0, "Provider not in escrow");

            // Calculate payment and slashing
            (uint256 payment, uint256 slashed, bool slaMet) = _calculatePayment(
                basePayment,
                m.uptimePercentage,
                m.replicationLagMs,
                m.dataCorrupted,
                m.maliciousDetected
            );

            // Pay provider
            if (payment > 0) {
                require(escrow.currentBalance >= payment, "Insufficient balance");
                escrow.token.safeTransfer(m.provider, payment);
                escrow.currentBalance -= payment;
                escrow.totalPaid += payment;
                totalPaidThisEpoch += payment;
            }

            // Apply slashing
            if (slashed > 0) {
                escrow.providerSlashes[m.provider] += slashed;
                escrow.totalSlashed += slashed;
                _slashProvider(m.provider, slashed);
            }

            // Record settlement
            EpochSettlement memory settlement = EpochSettlement({
                epoch: epoch,
                deploymentId: deploymentId,
                provider: m.provider,
                epochStart: epochStart,
                epochEnd: epochEnd,
                uptimePercentage: m.uptimePercentage,
                replicationLagMs: m.replicationLagMs,
                slaMet: slaMet,
                paymentAmount: payment,
                slashedAmount: slashed,
                settledAt: uint64(block.timestamp)
            });
            settlementHistory.push(settlement);

            emit EpochSettled(
                deploymentId,
                epoch,
                m.provider,
                payment,
                slashed,
                slaMet,
                uint64(block.timestamp)
            );
        }

        escrow.lastSettlementAt = uint64(block.timestamp);

        // Check if escrow depleted
        if (escrow.currentBalance == 0) {
            escrow.status = EscrowStatus.Depleted;
        }
    }

    /// @notice Internal payment calculation
    function _calculatePayment(
        uint256 basePayment,
        uint256 uptimePercentage,
        uint256 replicationLagMs,
        bool dataCorrupted,
        bool maliciousDetected
    ) internal view returns (uint256 payment, uint256 slashed, bool slaMet) {
        payment = basePayment;
        slashed = 0;
        slaMet = true;

        // Check uptime
        if (uptimePercentage < minUptimePercentage) {
            slaMet = false;
            uint256 slashAmount = (basePayment * uptimeSlashPercentage) / 10000;
            slashed += slashAmount;
            payment = payment > slashAmount ? payment - slashAmount : 0;
        }

        // Check replication lag
        if (replicationLagMs > maxReplicationLagMs) {
            slaMet = false;
            uint256 slashAmount = (basePayment * replicationLagSlashPercentage) / 10000;
            slashed += slashAmount;
            payment = payment > slashAmount ? payment - slashAmount : 0;
        }

        // Check data corruption
        if (dataCorrupted) {
            slaMet = false;
            uint256 slashAmount = (basePayment * dataCorruptionSlashPercentage) / 10000;
            slashed += slashAmount;
            payment = payment > slashAmount ? payment - slashAmount : 0;
        }

        // Check malicious behavior
        if (maliciousDetected) {
            slaMet = false;
            slashed = basePayment; // Full slash
            payment = 0;
        }

        return (payment, slashed, slaMet);
    }

    /// @notice Slash provider stake
    function _slashProvider(address provider, uint256 amount) internal {
        if (providerStakes[provider].amount >= amount) {
            providerStakes[provider].amount -= amount;
            providerStakes[provider].totalSlashed += amount;
        }
        
        emit ProviderSlashed(
            bytes32(0), // deploymentId not tracked per provider stake
            provider,
            amount,
            "SLA violation"
        );
    }

    // ============ Provider Stake Functions ============
    /// @notice Deposit stake as a provider
    function depositStake(IERC20 token, uint256 amount) external whenNotPaused {
        require(amount > 0, "Zero amount");
        token.safeTransferFrom(msg.sender, address(this), amount);
        
        ProviderStake storage stake = providerStakes[msg.sender];
        stake.amount += amount;
        stake.token = token.symbol();
    }

    /// @notice Lock stake for a period
    function lockStake(uint256 epochs) external whenNotPaused {
        ProviderStake storage stake = providerStakes[msg.sender];
        require(stake.amount > 0, "No stake to lock");
        require(epochs > 0, "Zero epochs");
        
        stake.locked = true;
        stake.lockExpiresAt = uint64(block.timestamp) + uint64(epochs) * uint64(epochDuration);
    }

    /// @notice Unlock stake (after lock period)
    function unlockStake() external whenNotPaused {
        ProviderStake storage stake = providerStakes[msg.sender];
        require(stake.locked, "Not locked");
        require(block.timestamp >= stake.lockExpiresAt, "Lock not expired");
        
        stake.locked = false;
        stake.lockExpiresAt = 0;
    }

    /// @notice Request withdrawal (starts lock period)
    function requestWithdrawal(uint256 amount) external whenNotPaused {
        ProviderStake storage stake = providerStakes[msg.sender];
        require(amount > 0, "Zero amount");
        require(amount <= stake.amount, "Insufficient stake");
        require(!stake.locked, "Already locked");
        
        stake.locked = true;
        stake.lockExpiresAt = uint64(block.timestamp) + withdrawalLockEpochs * epochDuration;
        
        emit WithdrawalRequested(msg.sender, amount, stake.lockExpiresAt);
    }

    /// @notice Execute withdrawal after lock period
    function executeWithdrawal(uint256 amount) external whenNotPaused {
        ProviderStake storage stake = providerStakes[msg.sender];
        require(amount > 0, "Zero amount");
        require(amount <= stake.amount, "Insufficient stake");
        require(stake.locked, "Not locked");
        require(block.timestamp >= stake.lockExpiresAt, "Lock not expired");
        
        stake.amount -= amount;
        stake.locked = false;
        stake.lockExpiresAt = 0;

        IERC20 token = escrows[bytes32(0)].token; // Would need proper token lookup
        token.safeTransfer(msg.sender, amount);

        emit WithdrawalExecuted(msg.sender, amount);
    }

    // ============ Escrow Lifecycle ============
    /// @notice Close escrow and refund remaining to developer
    function closeEscrow(bytes32 deploymentId) external whenNotPaused onlyDeveloper(deploymentId) {
        Escrow storage escrow = escrows[deploymentId];
        require(escrow.status != EscrowStatus.Closed, "Already closed");
        
        escrow.status = EscrowStatus.Closed;
        uint256 refund = escrow.currentBalance;
        escrow.currentBalance = 0;

        if (refund > 0) {
            escrow.token.safeTransfer(escrow.developer, refund);
        }

        emit EscrowClosed(deploymentId, refund);
    }

    /// @notice Raise dispute
    function raiseDispute(bytes32 deploymentId, address target, string calldata reason) external whenNotPaused {
        Escrow storage escrow = escrows[deploymentId];
        require(escrow.status == EscrowStatus.Active, "Escrow not active");
        
        escrow.status = EscrowStatus.Disputed;
        
        emit DisputeRaised(deploymentId, msg.sender, target, reason);
    }

    /// @notice Resolve dispute (owner only)
    function resolveDispute(bytes32 deploymentId, bool providerWon, uint256 compensation) external onlyOwner {
        Escrow storage escrow = escrows[deploymentId];
        require(escrow.status == EscrowStatus.Disputed, "No dispute");
        
        escrow.status = EscrowStatus.Active;
        
        if (providerWon && compensation > 0) {
            // Pay from escrow to provider
            require(escrow.currentBalance >= compensation, "Insufficient balance");
            escrow.token.safeTransfer(escrow.providers[0], compensation); // Simplified
            escrow.currentBalance -= compensation;
            escrow.totalPaid += compensation;
        }
        
        emit DisputeResolved(deploymentId, providerWon, compensation);
    }

    // ============ View Functions ============
    function getEscrow(bytes32 deploymentId) external view returns (
        address developer,
        address[] memory providers,
        address token,
        uint256 totalDeposited,
        uint256 currentBalance,
        uint32 epochsFunded,
        uint64 createdAt,
        EscrowStatus status
    ) {
        Escrow storage escrow = escrows[deploymentId];
        return (
            escrow.developer,
            escrow.providers,
            address(escrow.token),
            escrow.totalDeposited,
            escrow.currentBalance,
            escrow.epochsFunded,
            escrow.createdAt,
            escrow.status
        );
    }

    function getProviderStake(address provider) external view returns (
        uint256 amount,
        string memory token,
        bool locked,
        uint64 lockExpiresAt,
        uint256 totalSlashed
    ) {
        ProviderStake storage stake = providerStakes[provider];
        return (
            stake.amount,
            stake.token,
            stake.locked,
            stake.lockExpiresAt,
            stake.totalSlashed
        );
    }

    function getSettlementHistory(bytes32 deploymentId) external view returns (EpochSettlement[] memory) {
        // Filter by deploymentId
        uint256 count = 0;
        for (uint i = 0; i < settlementHistory.length; i++) {
            if (settlementHistory[i].deploymentId == deploymentId) {
                count++;
            }
        }
        
        EpochSettlement[] memory result = new EpochSettlement[](count);
        uint256 j = 0;
        for (uint i = 0; i < settlementHistory.length; i++) {
            if (settlementHistory[i].deploymentId == deploymentId) {
                result[j] = settlementHistory[i];
                j++;
            }
        }
        return result;
    }

    // ============ ERC20 Interface ============
    interface IERC20 {
        function safeTransferFrom(address from, address to, uint256 value) external;
        function safeTransfer(address to, uint256 value) external;
        function symbol() external view returns (string memory);
    }
}
