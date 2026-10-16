#pragma once
#include <QAbstractListModel>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

// Presentation-only role adapter. Typed Rust rows are converted once on the GUI
// thread. Append/update preserves delegates and scroll anchors during refresh.
class Rows : public QAbstractListModel {
    Q_OBJECT
    QML_ANONYMOUS
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
public:
    explicit Rows(QObject *parent = nullptr) : QAbstractListModel(parent) {}
    int rowCount(const QModelIndex &parent = {}) const override { return parent.isValid() ? 0 : static_cast<int>(m_rows.size()); }
    QVariant data(const QModelIndex &index, int role) const override {
        if (role != Qt::UserRole || !index.isValid() || index.row() < 0 || index.row() >= m_rows.size()) return {};
        return m_rows.at(index.row());
    }
    QHash<int, QByteArray> roleNames() const override { return {{Qt::UserRole, "row"}}; }
    Q_INVOKABLE QVariantMap get(int index) const { return index >= 0 && index < m_rows.size() ? m_rows.at(index) : QVariantMap{}; }
    void replace(QList<QVariantMap> rows) {
        const auto oldCount = m_rows.size();
        int shared = 0;
        while (shared < m_rows.size() && shared < rows.size() && m_rows[shared].value("id") == rows[shared].value("id")) ++shared;
        if (shared < m_rows.size()) {
            beginRemoveRows({}, shared, static_cast<int>(m_rows.size()) - 1);
            m_rows.erase(m_rows.begin() + shared, m_rows.end()); endRemoveRows();
        }
        for (int index = 0; index < shared; ++index) {
            if (m_rows[index] != rows[index]) { m_rows[index] = rows[index]; emit dataChanged(this->index(index), this->index(index)); }
        }
        if (shared < rows.size()) {
            beginInsertRows({}, shared, static_cast<int>(rows.size()) - 1);
            for (int index = shared; index < rows.size(); ++index) m_rows.push_back(std::move(rows[index]));
            endInsertRows();
        }
        if (oldCount != m_rows.size()) emit countChanged();
    }
signals:
    void countChanged();
private:
    QList<QVariantMap> m_rows;
};
