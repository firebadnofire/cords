#pragma once
#include <QAbstractListModel>
#include <QVariantMap>
#include <QHash>
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
    Q_INVOKABLE int indexOf(QString id) const {
        for (int index = 0; index < m_rows.size(); ++index)
            if (m_rows[index].value("id").toString() == id) return index;
        return -1;
    }
    void replace(QList<QVariantMap> rows) {
        if (m_rows == rows) return;
        emit aboutToChange();
        const auto oldCount = m_rows.size();
        QHash<QString, int> positions;
        for (int index = 0; index < rows.size(); ++index) positions.insert(rows[index].value("id").toString(), index);
        int oldStart = 0, newStart = 0, shared = 0;
        while (oldStart < m_rows.size() && !positions.contains(m_rows[oldStart].value("id").toString())) ++oldStart;
        if (oldStart < m_rows.size()) {
            newStart = positions.value(m_rows[oldStart].value("id").toString());
            while (oldStart + shared < m_rows.size() && newStart + shared < rows.size()
                   && m_rows[oldStart + shared].value("id") == rows[newStart + shared].value("id")) ++shared;
        }
        const auto tail = oldStart + shared;
        if (tail < m_rows.size()) {
            beginRemoveRows({}, tail, static_cast<int>(m_rows.size()) - 1);
            m_rows.erase(m_rows.begin() + tail, m_rows.end()); endRemoveRows();
        }
        if (oldStart > 0) {
            beginRemoveRows({}, 0, oldStart - 1);
            m_rows.erase(m_rows.begin(), m_rows.begin() + oldStart); endRemoveRows();
        }
        // Keep the shared contiguous IDs alive across prepends and edge pruning.
        for (int index = 0; index < shared; ++index) {
            if (m_rows[index] != rows[newStart + index]) { m_rows[index] = rows[newStart + index]; emit dataChanged(this->index(index), this->index(index)); }
        }
        if (newStart > 0) {
            beginInsertRows({}, 0, newStart - 1);
            for (int index = 0; index < newStart; ++index) m_rows.insert(index, std::move(rows[index]));
            endInsertRows();
        }
        if (newStart + shared < rows.size()) {
            beginInsertRows({}, newStart + shared, static_cast<int>(rows.size()) - 1);
            for (int index = newStart + shared; index < rows.size(); ++index) m_rows.push_back(std::move(rows[index]));
            endInsertRows();
        }
        if (oldCount != m_rows.size()) emit countChanged();
        emit changed();
    }
signals:
    void aboutToChange();
    void changed();
    void countChanged();
private:
    QList<QVariantMap> m_rows;
};
