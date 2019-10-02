import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import Cookies from 'js-cookie';
import { BrowserRouter as Router, Route, Switch, Link } from 'react-router-dom';
import TourAdd from './tour-add.jsx'
import TourNameEdit from './tour-rename.jsx'

export default class AuthenticatedApp extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isToursLoaded: false,
            tours: null
        }
    }
    componentDidMount() {
        document.title = "Tourcalc: List of tours"
        AppState.loadTours(this);
    }

    render() {
        if (!this.state.isToursLoaded) {
            return <div>Loading Tours ...</div>
        } else {
            return (
                <div>
                    Tours {
                        this.props.authData.type === 'Master'
                            ? <TourAdd buttonText="Add" actionButtonText="Add Tour" app={this} open={false} />
                            : <span />
                    }
                    {
                        this.state.tours.map((t, idx) => {
                            return (
                                <div key={'d' + idx}>
                                    {
                                        this.props.authData.type === 'Master' ? (
                                            <span key={'s' + idx} style={{ cursor: "pointer", borderStyle: 'ridge', fontSize: "xx-small" }} onClick={() => {
                                                if (window.confirm('Sure to delete tour ' + t.name + ' (id: ' + t.id + ')?')) {
                                                    AppState.deleteTour(this, t.id)
                                                        .then(() => { AppState.loadTours(this); })
                                                }
                                            }}>X</span>) : <span />
                                    }
                                    &nbsp;
                                    <u key={'u' + idx}><TourNameEdit key={'te' + idx} tourid={t.id} name={t.name} app={this} open={false} buttonText='Edit' actionButtonText="Change name" /></u>
                                    &nbsp;&nbsp;
                                    {idx + 1}.
                                <Link key={'l'+idx} to={'/tour/' + t.id}>{t.name}</Link>
                                </div>
                            )
                        })
                    }
                </div>
                )
        }
    }

};